use core::{
    arch::asm,
    cell::{BorrowMutError, LazyCell, RefCell, RefMut},
    time::Duration,
};

use crate::{timer, Pin, PinFsel};
use bcm2835_lpa::{Peripherals, UART1};

use crate::{dsb, interrupts::guard};

const ASSUMED_CLOCK_RATE: usize = 250_000_000;
const DESIRED_BAUD_RATE: usize = 115_200 * 8;

pub fn setup_uart(
    p14: Pin<14, { PinFsel::Unset }>,
    p15: Pin<15, { PinFsel::Unset }>,
    peripherals: &mut Peripherals,
) -> UartWriter {
    let _guard = guard::InterruptGuard::new();
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

static mut UART_WRITER: LazyCell<RefCell<Option<UartWriter>>> = LazyCell::new(|| None.into());

pub struct UartWriter {
    _p14: Pin<14, { PinFsel::Alt5 }>,
    _p15: Pin<15, { PinFsel::Alt5 }>,
}

impl core::fmt::Write for &mut UartWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        write_uart(s.as_bytes());
        Ok(())
    }
}

#[allow(static_mut_refs)]
pub fn store_uart(writer: UartWriter) {
    let guard = guard::InterruptGuard::new();
    unsafe { UART_WRITER.replace(Some(writer)) };
    drop(guard);
}

#[allow(static_mut_refs)]
pub unsafe fn uart_borrowed() -> bool {
    let guard = guard::InterruptGuard::new();
    let out = LazyCell::force(&crate::uart::UART_WRITER)
        .try_borrow_mut()
        .is_err();
    drop(guard);
    out
}

/// Panics if UART_WRITER is already borrowed.
#[allow(static_mut_refs)]
pub unsafe fn get_uart_mut() -> RefMut<'static, Option<UartWriter>> {
    let guard = guard::InterruptGuard::new();
    let out = LazyCell::force(&crate::uart::UART_WRITER).borrow_mut();
    drop(guard);
    out
}
#[allow(static_mut_refs)]
pub unsafe fn get_uart_mut_checked() -> Result<RefMut<'static, Option<UartWriter>>, BorrowMutError>
{
    let guard = guard::InterruptGuard::new();
    let out = LazyCell::force(&crate::uart::UART_WRITER).try_borrow_mut();
    drop(guard);
    out
}

/// This will error if the args cause an interrupt (like software interrupt).
#[macro_export]
macro_rules! dbg {
    ($( $args:expr),* ) => {
        let guard = crate::interrupts::guard::InterruptGuard::new();
        if let Some(mut w) = unsafe { $crate::uart::get_uart_mut() }.as_mut() {
            $(
                core::fmt::Write::write_fmt(&mut w, format_args!("[{}:{}:{}] ", file!(), line!(), column!())).unwrap();
                core::fmt::Write::write_fmt(&mut w, format_args!("{} = {:?}", stringify!($args), $args)).unwrap();
                core::fmt::Write::write_str(&mut w, "\n").unwrap();
            )*
        }
        drop(guard);
    };
}

/// This will error if the args cause an interrupt (like software interrupt).
#[macro_export]
macro_rules! print {
    ($( $args:tt)* ) => {
        let guard = crate::interrupts::guard::InterruptGuard::new();
        if let Some(mut w) = unsafe { $crate::uart::get_uart_mut() }.as_mut() {
            core::fmt::Write::write_fmt(&mut w, format_args!($($args)*)).unwrap();
        }
        drop(guard);
    };
}

/// This will error if the args cause an interrupt (like software interrupt).
#[macro_export]
macro_rules! println {
    ($( $args:tt)* ) => {
        let guard = crate::interrupts::guard::InterruptGuard::new();
        if let Some(mut w) = unsafe { $crate::uart::get_uart_mut() }.as_mut() {
            core::fmt::Write::write_fmt(&mut w, format_args!($($args)*)).unwrap();
            core::fmt::Write::write_str(&mut w, "\n").unwrap();
        }
        drop(guard);
    };
}

/// Software version of uart.
pub mod software {
    use crate::{
        cycle_counter::{delay, delay_until, read},
        interrupts::guard,
        Pin, PinFsel,
    };

    use super::DESIRED_BAUD_RATE;

    pub struct SWUart {
        p14: Pin<14, { PinFsel::Output }>,
    }

    impl SWUart {
        pub fn setup_output(p14: Pin<14, { PinFsel::Unset }>) -> Self {
            let _guard = guard::InterruptGuard::new();
            let mut p14 = p14.into_output();
            p14.write(true);
            Self { p14 }
        }

        pub fn write(&mut self, bytes: &[u8]) {
            // For a given baud rate, compute how many micro-seconds T you write each bit.
            // For example, for 115,200, this is: (1000*1000)/115200 = 8.68.
            // (NOTE: we will use cycles rather than micro-seconds since that is much easier
            // to make accurate. The A+ runs at 700MHz so that is 700 * 1000 * 1000 cycles
            // per second or about 6076 cycles per bit.)

            const ASSUMED_CLOCK_RATE: usize = 700_000_000;
            const MICROS_PER_CYCLE: usize = ASSUMED_CLOCK_RATE / 1_000_000;
            const CYCLES_PER_BIT: u32 = ((MICROS_PER_CYCLE * 1_000_000) / DESIRED_BAUD_RATE) as u32;
            let start = read();
            let mut desired = start;
            for byte in bytes {
                self.p14.write(false);
                desired = desired.wrapping_add(CYCLES_PER_BIT);
                delay_until(desired);
                // delay(CYCLES_PER_BIT);
                let mut v = *byte;
                for _ in 0..u8::BITS {
                    self.p14.write((v & 1) == 1);
                    desired = desired.wrapping_add(CYCLES_PER_BIT);
                    delay_until(desired);
                    v = v >> 1;
                }
                self.p14.write(true);
                desired = desired.wrapping_add(CYCLES_PER_BIT);
                delay_until(desired);
            }
        }

        pub fn consume(self) -> Pin<14, { PinFsel::Output }> {
            self.p14
        }
    }
}
