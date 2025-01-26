#![no_std] // don't link the Rust standard library
#![no_main] // disable all Rust-level entry points

mod profile;
mod setup;
mod timer;
mod uart;

use core::arch::asm;

use bcm2835_lpa::Peripherals;
use pi0_register::{Pin, PinFsel};
use setup::interrupts::{enable_interrupts, get_cnt, get_period};
use timer::{delay_ms, timer_get_usec};
use uart::{setup_uart, store_uart, uart_borrowed};

fn main() {
    let mut peripherals = unsafe { Peripherals::steal() };
    let pins = unsafe { pi0_register::get_pins() };
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

    //**************************************************
    // loop until we get N interrupts, tracking how many
    // times we can iterate.
    let start = timer_get_usec();

    //     // Q: what happens if you enable cache?  Why are some parts
    //     // the same, some change?
    //     //enable_cache();
    let mut iter = 0;
    let sum = 0;
    let n = 20;
    while (unsafe { get_cnt() } < n) {
        assert!(!unsafe { uart_borrowed() });
        // let _guard = interrupts::guard::InterruptGuard::new();
        // Q: if you comment this out?  why do #'s change?
        writeln!(
            "iter={}: cnt = {}, time between interrupts = {} usec ({:x})\n",
            iter,
            unsafe { get_cnt() },
            unsafe { get_period() },
            unsafe { get_period() },
        );
        iter += 1;
    }

    writeln!(
        "sum = {}, iter = {}, {}-{}",
        sum,
        iter,
        start,
        timer_get_usec(),
    );

    // writeln!("continued interrupts");

    writeln!("FINISHED RSSTART");
    let mut p0 = p0.into_output();
    p0.write(true);
    let mut set_on = false;
    for _ in 0..5 {
        p0.write(set_on);
        set_on = !set_on;
        delay_ms(100);
    }
}

/// Device synchronization barrier
fn dsb() {
    unsafe {
        asm!(
            "mcr p15, 0, {tmp}, c7, c10, 4",
            tmp = in(reg) 0,
        )
    }
}
