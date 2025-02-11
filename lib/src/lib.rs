#![no_std]
#![allow(incomplete_features)]
#![feature(adt_const_params)]
#![feature(generic_const_exprs)]
#![feature(unsized_const_params)]
#![allow(asm_sub_register)]

mod allocator;
pub mod caches;
mod critical_section;
pub mod cycle_counter;
pub mod gpio;
pub mod interrupts;
mod pin_array;
pub mod setup;
pub mod syscall;
pub mod thread;
pub mod timer;
pub mod uart;
pub use pin_array::get_pins;

extern crate alloc;

use core::arch::asm;

/// Device synchronization barrier
fn dsb() {
    unsafe {
        asm!(
            "mcr p15, 0, {tmp}, c7, c10, 4",
            tmp = in(reg) 0,
        )
    }
}
