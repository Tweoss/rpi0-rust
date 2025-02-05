#![no_std] // don't link the Rust standard library
#![no_main] // disable all Rust-level entry points
#![feature(alloc_error_handler)]

mod profile;
mod setup;

use bcm2835_lpa::Peripherals;
use pi0_lib::interrupts::enable_interrupts;
use pi0_lib::setup::rpi_reboot;
use pi0_lib::uart::software;
use pi0_lib::{get_pins, println, uart, Pin, PinFsel};
use pi0_lib::{interrupts, timer};
use uart::{setup_uart, store_uart};

fn main() {
    let mut peripherals = unsafe { Peripherals::steal() };
    let pins = unsafe { get_pins() };
    let (p14, pins): (Pin<14, { PinFsel::Unset }>, _) = pins.pluck();
    let (p15, _pins): (Pin<15, { PinFsel::Unset }>, _) = pins.pluck();

    let w = setup_uart(p14, p15, &mut peripherals);
    store_uart(w);

    enable_interrupts();

    println!("FINISHED RSSTART");
    println!("DONE!!!");

    rpi_reboot();
}
