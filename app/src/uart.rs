use core::arch::asm;

use bcm2835_lpa::{Peripherals, UART1};
use pi0_register::{Pin, PinFsel};

use crate::dsb;

const ASSUMED_CLOCK_RATE: usize = 250_000_000;
const DESIRED_BAUD_RATE: usize = 115_200;

pub fn setup_uart(p14: Pin<14, { PinFsel::Unset }>, peripherals: &mut Peripherals) -> UartWriter {
    // Set pin 14 to TX. Needs to happen before enabling uart.
    let p = p14.into_alt5();
    // TODO: UART input
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

    // TODO: disable interrupts
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
    // TODO: enable interrupts
    UartWriter { p14: p }
}

pub fn write_uart(bytes: &[u8]) {
    let uart = unsafe { UART1::steal() };
    for byte in bytes {
        // Wait until can write in a byte.
        while !uart.stat().read().tx_ready().bit_is_set() {
            unsafe { asm!("nop") }
        }
        uart.io().write(|w| unsafe { w.data().bits(*byte) });
    }
}

pub static mut UART_WRITER: Option<UartWriter> = None;

pub struct UartWriter {
    p14: Pin<14, { PinFsel::Alt5 }>,
}

impl UartWriter {
    pub fn consume(self) -> Pin<14, { PinFsel::Alt5 }> {
        self.p14
    }
}

impl core::fmt::Write for UartWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        write_uart(s.as_bytes());
        Ok(())
    }
}

#[macro_export]
macro_rules! dbg {
    ($w: expr, $( $args:expr),* ) => {
        $(
            core::fmt::Write::write_fmt($w, format_args!("[{}:{}:{}] ", file!(), line!(), column!())).unwrap();
            core::fmt::Write::write_fmt($w, format_args!("{} = {:?}", stringify!($args), $args)).unwrap();
            core::fmt::Write::write_str($w, "\n").unwrap();
        )*
    };
}

// TODO: add lock
/// Please drop ASAP.
pub unsafe fn get_uart_mut() -> Option<&'static mut UartWriter> {
    crate::uart::UART_WRITER.as_mut()
}

#[macro_export]
macro_rules! writeln {
    ($( $args:tt)* ) => {
        if let Some(w) = unsafe { $crate::uart::get_uart_mut() } {
            core::fmt::Write::write_fmt(w, format_args!($($args)*)).unwrap();
            core::fmt::Write::write_str(w, "\n").unwrap();
        }
    };
}
