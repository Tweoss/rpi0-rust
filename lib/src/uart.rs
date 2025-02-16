use core::{arch::asm, cell::RefCell, time::Duration};

use crate::{
    gpio::{Pin, PinFsel},
    timer,
};
use bcm2835_lpa::{Peripherals, UART1};
use critical_section::Mutex;

use crate::dsb;

const ASSUMED_CLOCK_RATE: usize = 250_000_000;
const DESIRED_BAUD_RATE: usize = 115_200 * 8;

pub fn setup_uart(
    p14: Pin<14, { PinFsel::Unset }>,
    p15: Pin<15, { PinFsel::Unset }>,
    peripherals: &mut Peripherals,
) -> UartWriter {
    critical_section::with(|_| {
        // Set pin 14 and 15 to alt5. Needs to happen before enabling uart.
        let p14 = p14.into_alt5();
        let p15 = p15.into_alt5();
        // Enable uart.
        dsb();
        peripherals
            .AUX
            .enables()
            .modify(|_, w| w.uart_1().set_bit());
        dsb();

        let uart = &peripherals.UART1;
        // Clear the TX/RX fifos.
        // Disable uart TX/RX and flow control.
        uart.cntl().write(|w| unsafe { w.bits(0) });

        uart.ier().write(|w| unsafe { w.bits(0) });
        uart.iir()
            .modify(|_, w| w.tx_ready().set_bit().data_ready().set_bit());

        // Set the baud rate.
        uart.baud().write(|w| unsafe {
            w.bits(
                (ASSUMED_CLOCK_RATE / 8 / DESIRED_BAUD_RATE - 1)
                    .try_into()
                    .unwrap(),
            )
        });
        uart.lcr().modify(|_, w| w.data_size()._8bit());
        uart.mcr().modify(|_, w| w.rts().clear_bit());
        // Enable TX/RX
        uart.cntl()
            .modify(|_, w| w.cts_enable().clear_bit().rts_enable().clear_bit());
        uart.cntl()
            .modify(|_, w| w.tx_enable().set_bit().rx_enable().set_bit());
        UartWriter {
            _p14: p14,
            _p15: p15,
        }
    })
}

pub fn read_uart_u32_timeout(timeout: Duration) -> Result<u32, ()> {
    let mut v: u32 = 0;
    let mut read_count = 0;
    let last_time = timer::timer_get_usec();

    loop {
        // Even if we started reading, once we hit timeout, return.
        if Duration::from_micros((timer::timer_get_usec() - last_time) as u64) > timeout {
            return Err(());
        }
        let mut buf = [0; 1];
        let read = read_uart(&mut buf);
        if read.is_empty() {
            continue;
        }
        read_count += 1;
        // Read in little endian.
        v = (v >> 8) + ((read[0] as u32) << 24);
        if read_count == 4 {
            break;
        }
    }
    Ok(v)
}

pub fn write_uart_u32(v: u32) {
    write_uart(&u32::to_le_bytes(v))
}

pub fn write_uart(bytes: &[u8]) {
    dsb();
    let uart = unsafe { UART1::steal() };
    for byte in bytes {
        // Wait until can write in a byte.
        while !uart.stat().read().tx_ready().bit_is_set() {
            unsafe { asm!("nop") }
        }
        uart.io().write(|w| unsafe { w.data().bits(*byte) });
    }
    // Wait till finished.
    while !uart.stat().read().tx_ready().bit_is_set() {
        unsafe { asm!("nop") }
    }
    dsb();
}

pub fn read_uart(dest: &mut [u8]) -> &[u8] {
    dsb();
    let uart = unsafe { UART1::steal() };
    for i in 0..dest.len() {
        // If no more to read, stop.
        if uart.stat().read().data_ready().bit_is_clear() {
            return &dest[0..i];
        }
        dest[i] = uart.io().read().bits() as u8;
    }
    dsb();
    return dest;
}

pub fn read_all_uart(dest: &mut [u8]) {
    dsb();
    let uart = unsafe { UART1::steal() };
    for byte in dest {
        // Wait until can read in a byte.
        while uart.stat().read().data_ready().bit_is_clear() {
            unsafe { asm!("nop") }
        }
        *byte = uart.io().read().bits() as u8;
    }
    dsb();
}

pub static UART_WRITER: Mutex<RefCell<Option<UartWriter>>> = Mutex::new(RefCell::new(None));

pub struct UartWriter {
    _p14: Pin<14, { PinFsel::Alt5 }>,
    _p15: Pin<15, { PinFsel::Alt5 }>,
}

impl UartWriter {
    pub unsafe fn steal() -> Self {
        Self {
            _p14: Pin::forge(),
            _p15: Pin::forge(),
        }
    }
}

impl core::fmt::Write for &mut UartWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        write_uart(s.as_bytes());
        Ok(())
    }
}

pub fn store_uart(writer: UartWriter) {
    critical_section::with(|cs| {
        UART_WRITER.borrow(cs).replace(Some(writer));
    })
}

/// This will error if the args cause an interrupt (like software interrupt).
#[macro_export]
macro_rules! dbg {
    ($( $args:expr),* ) => {
        critical_section::with(|cs| {
            let mut w = $crate::uart::UART_WRITER.borrow_ref_mut(cs);
            let Some(mut w) = w.as_mut() else {
                return;
            };
            $(
                core::fmt::Write::write_fmt(&mut w, format_args!("[{}:{}:{}] ", file!(), line!(), column!())).unwrap();
                core::fmt::Write::write_fmt(&mut w, format_args!("{} = {:?}", stringify!($args), $args)).unwrap();
                core::fmt::Write::write_str(&mut w, "\n").unwrap();
            )*
        });
    };
}

/// This will error if the args cause an interrupt (like software interrupt).
#[macro_export]
macro_rules! print {
    ($( $args:tt)* ) => {
        critical_section::with(|cs| {
            let mut w = $crate::uart::UART_WRITER.borrow_ref_mut(cs);
            let Some(mut w) = w.as_mut() else {
                return;
            };
            core::fmt::Write::write_fmt(&mut w, format_args!($($args)*)).unwrap();
        });
    };
}

/// This will error if the args cause an interrupt (like software interrupt).
#[macro_export]
macro_rules! println {
    ($( $args:tt)* ) => {
        critical_section::with(|cs| {
            let mut w = $crate::uart::UART_WRITER.borrow_ref_mut(cs);
            let Some(mut w) = w.as_mut() else {
                return;
            };
            core::fmt::Write::write_fmt(&mut w, format_args!($($args)*)).unwrap();
            core::fmt::Write::write_str(&mut w, "\n").unwrap();
        });
    };
}

#[macro_export]
macro_rules! steal_println {
    ($( $args:tt)* ) => {
        let mut w = &mut unsafe { $crate::uart::UartWriter::steal() };
        core::fmt::Write::write_fmt(&mut w, format_args!($($args)*)).unwrap();
        core::fmt::Write::write_str(&mut w, "\n").unwrap();
    };
}

/// Software version of uart.
pub mod software {

    use crate::{
        cycle_counter::{delay_until, read},
        gpio::{valid_pin, If, Pin, PinFsel, True},
        timer::delay_ms,
    };

    pub struct SWUart<const INDEX: usize>
    where
        If<{ valid_pin(INDEX) }>: True,
    {
        pin: Pin<INDEX, { PinFsel::Output }>,
    }

    impl<const INDEX: usize> SWUart<INDEX>
    where
        If<{ valid_pin(INDEX) }>: True,
    {
        pub fn setup_output(pin: Pin<INDEX, { PinFsel::Unset }>) -> Self {
            critical_section::with(|_| {
                let mut pin = pin.into_output();
                pin.write(true);
                delay_ms(10);
                Self { pin }
            })
        }

        pub fn write(&mut self, bytes: &[u8]) {
            // For a given baud rate, compute how many micro-seconds T you write each bit.
            // For example, for 115,200, this is: (1000*1000)/115200 = 8.68.
            // (NOTE: we will use cycles rather than micro-seconds since that is much easier
            // to make accurate. The A+ runs at 700MHz so that is 700 * 1000 * 1000 cycles
            // per second or about 6076 cycles per bit.)

            // Different clocks: here is 700 MHz, the uart1 is 250 MHz.
            // Lower baud rate here (for the demo) because interrupt code is not
            // fast enough to keep up with 8 times faster than 115200.
            const ASSUMED_CLOCK_RATE: usize = 700_000_000;
            const CYCLES_PER_MICRO: usize = ASSUMED_CLOCK_RATE / 1_000_000;
            const DESIRED_BAUD_RATE: usize = 115_200;

            const CYCLES_PER_BIT: u32 = ((CYCLES_PER_MICRO * 1_000_000) / DESIRED_BAUD_RATE) as u32;
            let start = read();
            let mut desired = start;
            for byte in bytes {
                self.pin.write(false);
                desired = desired.wrapping_add(CYCLES_PER_BIT);
                delay_until(desired);
                let mut v = *byte;
                for _ in 0..u8::BITS {
                    self.pin.write((v & 1) == 1);
                    desired = desired.wrapping_add(CYCLES_PER_BIT);
                    delay_until(desired);
                    v = v >> 1;
                }
                self.pin.write(true);
                desired = desired.wrapping_add(CYCLES_PER_BIT);
                delay_until(desired);
            }
        }

        pub fn consume(self) -> Pin<INDEX, { PinFsel::Output }> {
            self.pin
        }
    }
}
