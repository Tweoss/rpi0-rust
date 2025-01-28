use core::{
    arch::asm,
    cell::{BorrowMutError, LazyCell, RefCell, RefMut},
};

use bcm2835_lpa::{Peripherals, UART1};
use pi0_register::{Pin, PinFsel};

use crate::{dsb, setup::interrupts::guard};

const ASSUMED_CLOCK_RATE: usize = 250_000_000;
const DESIRED_BAUD_RATE: usize = 115_200;

pub fn setup_uart(p14: Pin<14, { PinFsel::Unset }>, peripherals: &mut Peripherals) -> UartWriter {
    let _guard = guard::InterruptGuard::new();
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
    UartWriter { _p14: p }
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

static mut UART_WRITER: LazyCell<RefCell<Option<UartWriter>>> = LazyCell::new(|| None.into());

pub struct UartWriter {
    _p14: Pin<14, { PinFsel::Alt5 }>,
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
    LazyCell::force(&crate::uart::UART_WRITER)
        .try_borrow_mut()
        .is_err()
}

/// Panics if UART_WRITER is already borrowed.
#[allow(static_mut_refs)]
pub unsafe fn get_uart_mut() -> RefMut<'static, Option<UartWriter>> {
    LazyCell::force(&crate::uart::UART_WRITER).borrow_mut()
}
#[allow(static_mut_refs)]
pub unsafe fn get_uart_mut_checked() -> Result<RefMut<'static, Option<UartWriter>>, BorrowMutError>
{
    LazyCell::force(&crate::uart::UART_WRITER).try_borrow_mut()
}

/// This will error if the args cause an interrupt (like software interrupt).
#[macro_export]
macro_rules! dbg {
    ($( $args:expr),* ) => {
        let guard = crate::setup::interrupts::guard::InterruptGuard::new();
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
        let guard = crate::setup::interrupts::guard::InterruptGuard::new();
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
        let guard = crate::setup::interrupts::guard::InterruptGuard::new();
        if let Some(mut w) = unsafe { $crate::uart::get_uart_mut() }.as_mut() {
            core::fmt::Write::write_fmt(&mut w, format_args!($($args)*)).unwrap();
            core::fmt::Write::write_str(&mut w, "\n").unwrap();
        }
        drop(guard);
    };
}
