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
    // #   define N 20
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

    // writeln!("sum = {}, iter = {}", sum, iter);

    // writeln!("continued interrupts");

    // writeln!("FINISHED RSSTART");
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

/// Demos loopback from gpio 9-> 10
/// blinks leds on gpio 26 and gpio 0 in an alteranting pattern
fn alt_blink(p: &Peripherals) -> ! {
    p.GPIO.gpfsel0().modify(|_, w| w.fsel9().output());
    p.GPIO.gpfsel1().modify(|_, w| w.fsel10().input());

    p.GPIO.gpfsel2().modify(|_, w| w.fsel26().output());
    p.GPIO.gpfsel0().modify(|_, w| w.fsel0().output());

    unsafe { p.GPIO.gpclr0().write_with_zero(|w| w.clr26().bit(true)) };

    let mut pin_9_on = false;
    loop {
        let read_in = p.GPIO.gplev0().read().lev10().bit();

        if !read_in {
            unsafe { p.GPIO.gpset0().write_with_zero(|w| w.set0().set_bit()) };
        } else {
            unsafe {
                p.GPIO
                    .gpclr0()
                    .write_with_zero(|w| w.clr0().clear_bit_by_one())
            };
        }

        for _ in 0..100000 {
            unsafe { asm!("nop") }
        }
        pin_9_on = !pin_9_on;

        if pin_9_on {
            unsafe {
                p.GPIO
                    .gpset0()
                    .write_with_zero(|w| w.set9().bit(pin_9_on).set26().set_bit())
            };
            unsafe { p.GPIO.gpset0().write_with_zero(|w| w.set26().set_bit()) };
        } else {
            unsafe {
                p.GPIO
                    .gpclr0()
                    .write_with_zero(|w| w.clr9().clear_bit_by_one().clr26().clear_bit_by_one())
            };
            unsafe {
                p.GPIO
                    .gpclr0()
                    .write_with_zero(|w| w.clr26().clear_bit_by_one())
            };
        }
    }
}
