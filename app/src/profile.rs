extern crate alloc;

use alloc::vec::Vec;
use core::{arch::asm, cell::RefCell};
use critical_section::Mutex;
use pi0_lib::{
    dbg,
    interrupts::{
        self, interrupts_enabled, register_interrupt_handler, remove_interrupt_handler,
        timer_clear, timer_initialized,
    },
    print,
};

use crate::{println, profile, timer};

extern "C" {
    static mut __code_start__: u8;
    static mut __code_end__: u8;
    static mut __data_start__: u8;
    static mut __data_end__: u8;
    static mut __bss_start__: u8;
    static mut __bss_end__: u8;
}

static GPROF: Mutex<RefCell<Option<Gprof>>> = Mutex::new(RefCell::new(None));

pub struct Gprof {
    buffer: Vec<u32>,
    pc_start: usize,
}

pub fn store_gprof(gprof: Gprof) {
    critical_section::with(|cs| GPROF.replace(cs, Some(gprof)));
}

impl Gprof {
    pub unsafe fn gprof_init() -> Self {
        const START: usize = 0x8000;
        let end = (&raw const __code_end__);
        println!("using start {:#08x}, end {:#08x}", START, end as usize);
        let mut buffer = Vec::new();
        buffer.resize(((end as usize) - START) / (size_of::<usize>()), 0);
        Gprof {
            buffer,
            pc_start: START,
        }
    }

    pub fn gprof_inc(pc: usize) {
        critical_section::with(|cs| {
            let mut gprof = GPROF.borrow_ref_mut(cs);
            let Some(gprof) = gprof.as_mut() else {
                return;
            };
            assert!(pc >= gprof.pc_start);
            gprof.buffer[(pc - gprof.pc_start) / size_of::<usize>()] += 1;
        });
    }

    pub fn gprof_dump() {
        critical_section::with(|cs| {
            let gprof = GPROF.borrow(cs).borrow();
            let Some(gprof) = gprof.as_ref() else {
                return;
            };
            let total: u32 = gprof.buffer.iter().sum();
            println!(
                "program counts (from {:#08x} to {:#08x})",
                gprof.pc_start,
                gprof.pc_start + gprof.buffer.len() * 4
            );
            for (i, c) in gprof.buffer.iter().enumerate() {
                if *c > 0 {
                    print!("{:#08x}:{c},", gprof.pc_start + i * size_of::<usize>());
                }
            }
            println!("");

            println!("Total count: {}", total);
        })
    }
}

#[allow(unused)]
pub fn demo() {
    let gprof = unsafe { Gprof::gprof_init() };
    store_gprof(gprof);
    let handler = register_interrupt_handler(alloc::boxed::Box::new(|_, pc| {
        Gprof::gprof_inc(pc as usize);
    }));

    assert!(interrupts_enabled(), "need interrupts to run gprof");
    assert!(timer_initialized(), "need timer to run gprof");
    let start = timer::timer_get_usec();

    let mut iter = 0;
    let sum = 0;

    let n = 10;
    while (unsafe { interrupts::get_cnt() } < n) {
        fib(0x20);
        println!(
            "iter={}: cnt = {}, time between interrupts = {} usec ({:x})",
            iter,
            unsafe { interrupts::get_cnt() },
            unsafe { interrupts::get_period() },
            unsafe { interrupts::get_period() },
        );
        iter += 1;
    }
    remove_interrupt_handler(handler);

    println!(
        "sum = {}, iter = {}, {}-{}",
        sum,
        iter,
        start,
        timer::timer_get_usec(),
    );
    interrupts::disable_interrupts();
    profile::Gprof::gprof_dump();
}

fn fib(n: u32) -> u32 {
    match n {
        0 => 1,
        1 => 1,
        n => fib(n - 1) + fib(n - 2),
    }
}
