use core::{
    cell::{LazyCell, RefCell, RefMut},
    slice,
};

use crate::{print, println, setup::interrupts::guard::InterruptGuard};

extern "C" {
    static mut __code_start__: u8;
    static mut __code_end__: u8;
    static mut __data_start__: u8;
    static mut __data_end__: u8;
    static mut __bss_start__: u8;
    static mut __bss_end__: u8;
}

const HEAP_START: usize = 1024 * 1024;
const ALLOCATED_AMOUNT: usize = 2 * 1024 * 1024;

static mut GPROF: LazyCell<RefCell<Option<Gprof>>> = LazyCell::new(|| None.into());

pub struct Gprof {
    buffer: &'static mut [u32],
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
        println!(
            "code: {:#08x}-{:#08x}",
            &raw const __code_start__ as usize, &raw const __code_end__ as usize
        );
        println!(
            "data: {:#08x}-{:#08x}",
            &raw const __data_start__ as usize, &raw const __data_end__ as usize
        );
        println!(
            "bss: {:#08x}-{:#08x}",
            &raw const __bss_start__ as usize, &raw const __bss_end__ as usize
        );
        // Just take a segment of memory and pretend we own it.
        Gprof {
            buffer: unsafe {
                slice::from_raw_parts_mut(
                    HEAP_START as *mut u32,
                    ((end as usize) - START) / (size_of::<usize>()),
                )
            },
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
        // self.buffer[pc - self.pc_start] += 1;
    }
}
