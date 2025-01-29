extern crate alloc;

use alloc::vec::Vec;
use core::cell::{LazyCell, RefCell, RefMut};

use crate::{
    print, println, profile,
    setup::{self, interrupts::guard::InterruptGuard},
    timer, uart,
};

extern "C" {
    static mut __code_start__: u8;
    static mut __code_end__: u8;
    static mut __data_start__: u8;
    static mut __data_end__: u8;
    static mut __bss_start__: u8;
    static mut __bss_end__: u8;
}

static mut GPROF: LazyCell<RefCell<Option<Gprof>>> = LazyCell::new(|| None.into());

pub struct Gprof {
    buffer: Vec<u32>,
    pc_start: usize,
}

#[allow(static_mut_refs)]
pub fn store_gprof(gprof: Gprof) {
    let guard = InterruptGuard::new();
    unsafe { GPROF.replace(Some(gprof)) };
    drop(guard);
}

/// Panics if already borrowed.
#[allow(static_mut_refs)]
pub unsafe fn get_gprof_mut() -> RefMut<'static, Option<Gprof>> {
    LazyCell::force(&GPROF).borrow_mut()
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

    pub fn gprof_inc(&mut self, pc: usize) {
        assert!(pc >= self.pc_start);
        self.buffer[(pc - self.pc_start) / size_of::<usize>()] += 1;
    }

    pub fn gprof_dump(&self) {
        let _guard = InterruptGuard::new();
        let total: u32 = self.buffer.iter().sum();
        println!(
            "program counts (from {:#08x} to {:#08x})",
            self.pc_start,
            self.pc_start + self.buffer.len() * 4
        );
        for (i, c) in self.buffer.iter().enumerate() {
            if *c > 0 {
                print!("{:#08x}:{c},", self.pc_start + i * size_of::<usize>());
            }
        }
        println!("");

        println!("Total count: {}", total);
    }
}

pub fn run() {
    // MAKE SURE TO TIMER_INIT
    let start = timer::timer_get_usec();

    let mut iter = 0;
    let sum = 0;

    let n = 2000;
    while (unsafe { setup::interrupts::get_cnt() } < n) {
        assert!(!unsafe { uart::uart_borrowed() });
        println!(
            "iter={}: cnt = {}, time between interrupts = {} usec ({:x})",
            iter,
            unsafe { setup::interrupts::get_cnt() },
            unsafe { setup::interrupts::get_period() },
            unsafe { setup::interrupts::get_period() },
        );
        iter += 1;
    }

    println!(
        "sum = {}, iter = {}, {}-{}",
        sum,
        iter,
        start,
        timer::timer_get_usec(),
    );
    setup::interrupts::disable_interrupts();
    unsafe { profile::get_gprof_mut().as_mut().unwrap().gprof_dump() };
}
