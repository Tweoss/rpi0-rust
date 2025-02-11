#![no_std] // don't link the Rust standard library
#![no_main] // disable all Rust-level entry points
#![feature(alloc_error_handler)]
#![allow(asm_sub_register)]

extern crate alloc;

mod profile;
mod setup;

use core::borrow::BorrowMut;
use core::cell::RefCell;

use alloc::boxed::Box;
use bcm2835_lpa::Peripherals;
use critical_section::Mutex;
use pi0_lib::interrupts::{enable_interrupts, gpio_interrupts_init, register_interrupt_handler};
use pi0_lib::setup::rpi_reboot;
use pi0_lib::timer::delay_ms;
use pi0_lib::{cycle_counter, dbg, interrupts, timer};
use pi0_lib::{
    get_pins,
    gpio::{Pin, PinFsel},
    println, uart,
};
use uart::{setup_uart, store_uart};

static LAST_CYCLE_COUNT: Mutex<RefCell<u32>> = Mutex::new(RefCell::new(0));

fn main() {
    let mut peripherals = unsafe { Peripherals::steal() };
    let pins = unsafe { get_pins() };
    let (p14, pins): (Pin<14, { PinFsel::Unset }>, _) = pins.pluck();
    let (p15, pins): (Pin<15, { PinFsel::Unset }>, _) = pins.pluck();

    let w = setup_uart(p14, p15, &mut peripherals);
    store_uart(w);

    let (p12, pins): (Pin<12, { PinFsel::Unset }>, _) = pins.pluck();
    let (p13, _pins): (Pin<13, { PinFsel::Unset }>, _) = pins.pluck();

    let mut p12 = p12.into_output();
    let p13 = p13.into_input();
    p13.set_rising_detection(true);
    p13.set_falling_detection(true);
    register_interrupt_handler(Box::new(move |cs, _| {
        if p13.event_detected() {
            let mut last = LAST_CYCLE_COUNT.borrow_ref_mut(cs);
            let current = cycle_counter::read();
            println!("{}", current - *last);
            *last = current;
            p13.clear_event();
        }
    }));

    enable_interrupts();

    let mut bit = false;
    for i in 0..10 {
        delay_ms(400);
        println!("running {i} at cycle {}", cycle_counter::read());

        critical_section::with(|cs| {
            LAST_CYCLE_COUNT.replace(cs, cycle_counter::read());
        });
        p12.write(!bit);
        bit = !bit;
    }

    // pi0_lib::syscall::demo();

    // profile::demo();

    println!("FINISHED RSSTART");
    println!("DONE!!!");

    rpi_reboot();
}
