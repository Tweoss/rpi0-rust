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
use pi0_lib::debug::{set_breakpoint_status, BreakpointStatus, WatchpointStatus};
use pi0_lib::interrupts::enable_interrupts;
use pi0_lib::setup::rpi_reboot;
use pi0_lib::{caches, dbg, timer};
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

fn main() {
    let mut peripherals = unsafe { Peripherals::steal() };
    let pins = unsafe { get_pins() };
    let (p14, pins): (Pin<14, { PinFsel::Unset }>, _) = pins.pluck();
    let (p15, _pins): (Pin<15, { PinFsel::Unset }>, _) = pins.pluck();
    let w = setup_uart(p14, p15, &mut peripherals);
    store_uart(w);

    caches::enable();

    enable_interrupts();

    // let mut x: Box<u32> = Box::new(0);
    // pi0_lib::debug::set_watchpoint_address(((x.as_ref()) as *const u32) as u32);
    // pi0_lib::debug::set_watchpoint_status(WatchpointStatus::Enabled {
    //     load: true,
    //     store: true,
    // });
    // unsafe { (x.as_mut() as *mut u32).write_volatile(3) };
    // let a = unsafe { (x.as_mut() as *mut u32).read_volatile() };
    // unsafe { (x.as_mut() as *mut u32).write_volatile(1) };
    // dbg!(a);

    // print();
    // let addr = (print as *const ()) as u32;
    // println!("addr: {:04x}", addr);
    // // set_breakpoint_address((print as *const ()) as u32);
    // set_breakpoint_address(0);
    // set_breakpoint_status(BreakpointStatus::Enabled { matching: false });

    // extern "C" fn run_fib() -> ! {
    //     let a = 1;
    //     let mut b = a + a;
    //     b = b * b << a + 23;
    //     for i in 0..2 {
    //         println!("fib {i} = {}", fib(i));
    //     }
    //     core::hint::black_box(b);
    //     panic!("");
    //     // set_breakpoint_status(BreakpointStatus::Disabled);
    //     //     //
    //     //     // // set_breakpoint_status(BreakpointStatus::Disabled);
    //     //     loop {}
    //     //     // rpi_reboot();
    // }
    // #[no_mangle]
    // pub extern "C" fn fib(x: usize) -> usize {
    //     let mut a = 1;
    //     let mut b = 1;
    //     for _ in 0..x {
    //         let c = a;
    //         a += b;
    //         b = c;
    //     }
    //     b
    // }
    // // // set_breakpoint_status(BreakpointStatus::Disabled);
    // run_user_code(run_fib);
    pi0_lib::syscall::demo();
    // print();
    // *x = 1;
    // core::hint::black_box(*x);
    // *x *= 2 * *x;
    // *x *= *x * *x;
    // core::hint::black_box(x);
    set_breakpoint_status(BreakpointStatus::Disabled);
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
