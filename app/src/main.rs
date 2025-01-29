#![no_std] // don't link the Rust standard library
#![no_main] // disable all Rust-level entry points
#![feature(alloc_error_handler)]

mod allocator;
mod profile;
mod setup;
mod syscall;
mod thread;
mod timer;
mod uart;

use core::arch::asm;

use bcm2835_lpa::Peripherals;
use pi0_register::{Pin, PinFsel};
use setup::{interrupts::enable_interrupts, rpi_reboot};
use syscall::{syscall_error, syscall_hello};
use thread::{thread_exit, thread_yield};
use timer::delay_ms;
use uart::{setup_uart, store_uart};

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

    extern "C" fn thread_0(arg: *mut u32) {
        println!("thread_0 has arg: {}", unsafe { *arg });
        println!("thread_0 yields now");
        thread_yield();
        println!("thread_0 exits now");
        thread_exit(1);
    }
    extern "C" fn thread_1(arg: *mut u32) {
        println!("thread_1 has arg: {}", unsafe { *arg });
    }
    let mut arg = 42;
    thread::fork(thread_0, &mut arg);
    thread::start();
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

extern "C" fn umain() -> ! {
    println!("REACHED UMAIN");
    let v = syscall_hello();
    println!("syscall_hello {}", v);
    let v = syscall_error();
    println!("syscall_error {}", v);
    println!("FINISHED UMAIN");
    let mut p0 = unsafe { Pin::<0, { PinFsel::Unset }>::forge().into_output() };
    p0.write(true);
    let mut set_on = false;
    for _ in 0..5 {
        p0.write(set_on);
        set_on = !set_on;
        delay_ms(100);
    }
    rpi_reboot()
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
