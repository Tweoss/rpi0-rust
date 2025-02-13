#![no_std] // don't link the Rust standard library
#![no_main] // disable all Rust-level entry points
#![feature(alloc_error_handler)]
#![allow(asm_sub_register)]

extern crate alloc;

mod profile;
mod setup;

use core::cell::RefCell;

use alloc::boxed::Box;
use bcm2835_lpa::Peripherals;
use critical_section::Mutex;
use heapless::Deque;
use pi0_lib::interrupts::{enable_interrupts, register_interrupt_handler};
use pi0_lib::setup::rpi_reboot;
use pi0_lib::timer::delay_ms;
use pi0_lib::uart::software;
use pi0_lib::{caches, cycle_counter, dbg, timer};
use pi0_lib::{
    get_pins,
    gpio::{Pin, PinFsel},
    println, uart,
};
use uart::{setup_uart, store_uart};

static CYCLE_COUNTS: Mutex<RefCell<Deque<u32, 20>>> = Mutex::new(RefCell::new(Deque::new()));
static PIN_VALUES: Mutex<RefCell<Deque<bool, 20>>> = Mutex::new(RefCell::new(Deque::new()));

fn main() {
    let mut peripherals = unsafe { Peripherals::steal() };
    let pins = unsafe { get_pins() };
    let (p14, pins): (Pin<14, { PinFsel::Unset }>, _) = pins.pluck();
    let (p15, pins): (Pin<15, { PinFsel::Unset }>, _) = pins.pluck();
    let w = setup_uart(p14, p15, &mut peripherals);
    store_uart(w);

    let (p12, pins): (Pin<12, { PinFsel::Unset }>, _) = pins.pluck();
    let (p13, _pins): (Pin<13, { PinFsel::Unset }>, _) = pins.pluck();

    caches::enable();
    let mut swuart = software::SWUart::<12>::setup_output(p12);

    let p13 = p13.into_input();
    p13.set_rising_detection(true);
    p13.set_falling_detection(true);
    p13.clear_event();

    register_interrupt_handler(Box::new(move |cs, _| {
        if p13.event_detected() {
            let mut counts = CYCLE_COUNTS.borrow_ref_mut(cs);
            counts.push_back(cycle_counter::read());
            let mut pin_values = PIN_VALUES.borrow_ref_mut(cs);
            pin_values.push_back(p13.read());
            p13.clear_event();
            return true;
        }
        false
    }));

    enable_interrupts();

    dbg!(cycle_counter::read());

    for _ in 0..8 {
        swuart.write(&[0b0101_0101]);
        delay_ms(100);
        println!("{}", cycle_counter::read());

        critical_section::with(|cs| {
            let mut counts = CYCLE_COUNTS.borrow_ref_mut(cs);
            let mut bits = PIN_VALUES.borrow_ref_mut(cs);
            println!("#events = {}", counts.len() / 8);
            let mut start = counts.pop_front().unwrap();
            println!("bit = {}", bits.pop_front().unwrap());
            while let Some(cycle) = counts.pop_front() {
                let read = bits.pop_front().unwrap();
                println!("wait = {}, read = {read}", cycle - start);
                start = cycle;
            }
        });
    }

    // pi0_lib::syscall::demo();

    // profile::demo();

    println!("FINISHED RSSTART");
    println!("DONE!!!");

    rpi_reboot();
}
