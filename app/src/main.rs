#![no_std] // don't link the Rust standard library
#![no_main] // disable all Rust-level entry points
#![feature(alloc_error_handler)]
#![allow(asm_sub_register)]

extern crate alloc;

mod profile;
mod setup;

use core::cell::RefCell;

use bcm2835_lpa::Peripherals;
use critical_section::Mutex;
use heapless::Deque;
use pi0_lib::interrupts::{enable_interrupts, run_user_code};
use pi0_lib::setup::rpi_reboot;
use pi0_lib::{dbg, timer};
use pi0_lib::{
    get_pins,
    gpio::{Pin, PinFsel},
    println, uart,
};
use uart::{setup_uart, store_uart};

static CYCLE_COUNTS: Mutex<RefCell<Deque<u32, 20>>> = Mutex::new(RefCell::new(Deque::new()));
static PIN_VALUES: Mutex<RefCell<Deque<bool, 20>>> = Mutex::new(RefCell::new(Deque::new()));

extern "C" fn print() {
    dbg!("printing hullo");
}

// first, run a, run b, collect output state
//
// then, enable mismatch breakpoint, run a single instruction, save registers in static
// from handler, run b fully, (return from handler will restore registers), then return to a
// run a to completion, compare output state
//
//
//

// into user mode, enable breakpoint.
//

fn main() {
    // caches::enable();

    enable_interrupts();

    // pi0_lib::debug::demo();

    // let mut x: alloc::boxed::Box<u32> = alloc::boxed::Box::new(0);
    // pi0_lib::coprocessor::set_watchpoint_address(((x.as_ref()) as *const u32) as u32);
    // pi0_lib::coprocessor::set_watchpoint_status(pi0_lib::coprocessor::WatchpointStatus::Enabled {
    //     load: true,
    //     store: true,
    // });
    // unsafe { (x.as_mut() as *mut u32).write_volatile(3) };
    // let a = unsafe { (x.as_mut() as *mut u32).read_volatile() };
    // unsafe { (x.as_mut() as *mut u32).write_volatile(1) };
    // dbg!(a);

    // print();
    // // set_breakpoint_address((print as *const ()) as u32);
    // pi0_lib::debug::set_watchpoint_status(WatchpointStatus::Disabled);
    // set_breakpoint_address(0);
    // set_breakpoint_status(BreakpointStatus::Enabled { matching: false });

    // // // set_breakpoint_status(BreakpointStatus::Disabled);
    // run_user_code(run_fib);
    // pi0_lib::syscall::demo();
    // print();
    // *x = 1;
    // core::hint::black_box(*x);
    // *x *= 2 * *x;
    // *x *= *x * *x;
    // core::hint::black_box(x);
    // set_breakpoint_status(BreakpointStatus::Disabled);
    // print();
    // print();

    // core::hint::black_box(x);

    // unsafe { core::arch::asm!("ldr {},[{}]", out(reg) x, in(reg) 0) };

    // pi0_lib::syscall::demo();

    // profile::demo();

    println!("FINISHED RSSTART");
    println!("DONE!!!");

    rpi_reboot();
}
