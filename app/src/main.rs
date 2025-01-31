#![no_std] // don't link the Rust standard library
#![no_main] // disable all Rust-level entry points
#![feature(alloc_error_handler)]

mod profile;
mod setup;
mod thread;

use bcm2835_lpa::Peripherals;
use pi0_lib::interrupts::enable_interrupts;
use pi0_lib::setup::rpi_reboot;
use pi0_lib::{get_pins, println, uart, Pin, PinFsel};
use pi0_lib::{interrupts, timer};
use timer::delay_ms;
use uart::{setup_uart, store_uart};

fn main() {
    let mut peripherals = unsafe { Peripherals::steal() };
    let pins = unsafe { get_pins() };
    let (p0, pins): (Pin<0, { PinFsel::Unset }>, _) = pins.pluck();
    let (p14, _pins): (Pin<14, { PinFsel::Unset }>, _) = pins.pluck();
    let w = setup_uart(p14, &mut peripherals);
    store_uart(w);

    let mut p0 = p0.into_output();
    p0.write(true);
    let mut set_on = false;
    for _ in 0..5 {
        p0.write(set_on);
        set_on = !set_on;
        delay_ms(100);
    }

    enable_interrupts();
    // thread::demo();
    // syscall::demo();

    // fork(umain, arg);

    println!("FINISHED RSSTART");
    println!("DONE!!!");

    let mut p0 = p0.into_output();
    p0.write(true);
    let mut set_on = false;
    for _ in 0..5 {
        p0.write(set_on);
        set_on = !set_on;
        delay_ms(100);
    }

    rpi_reboot();
}
