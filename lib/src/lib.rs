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
pub mod debug;
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
/// Flush the instruction prefetching pipeline.
#[allow(unused)]
fn prefetch_flush() {
    unsafe { asm!( "mcr p15, 0, {}, c7, c5, 4", in(reg) 0, ) }
}

#[macro_export]
/// Read from coprocessor register.
macro_rules! cp_asm_get {
    ($name: ident, $coprocessor: ident, $opcode1: literal, $Crn: ident, $Crm: ident, $opcode2: literal ) => {
        #[inline]
        fn $name() -> u32 {
            let v;
            unsafe {
                core::arch::asm!(
                    concat!(
                        "mrc ",
                        stringify!($coprocessor),
                        ", ",
                        stringify!($opcode1),
                        ", {v}, ",
                        stringify!($Crn),
                        ", ",
                        stringify!($Crm),
                        ", ",
                        stringify!($opcode2)
                    ),
                    v = out(reg) v
                );
            }
            v
        }
    };
}

#[macro_export]
/// Write to coprocessor register with prefetch flush.
macro_rules! cp_asm_set {
    ($name: ident, $coprocessor: ident, $opcode1: literal, $Crn: ident, $Crm: ident, $opcode2: literal ) => {
        #[inline]
        fn $name(v: u32) {
            unsafe {
                core::arch::asm!(
                    concat!(
                        "mcr ",
                        stringify!($coprocessor),
                        ", ",
                        stringify!($opcode1),
                        ", {v}, ",
                        stringify!($Crn),
                        ", ",
                        stringify!($Crm),
                        ", ",
                        stringify!($opcode2)
                    ),
                    v = in(reg) v
                );
            }
            // Prefetch flush.
            crate::prefetch_flush();
        }
    };
}

#[macro_export]
/// Write to coprocessor register without prefetch flush.
macro_rules! cp_asm_set_raw {
    ($name: ident, $coprocessor: ident, $opcode1: literal, $Crn: ident, $Crm: ident, $opcode2: literal ) => {
        #[inline]
        fn $name(v: u32) {
            unsafe {
                core::arch::asm!(
                    concat!(
                        "mcr ",
                        stringify!($coprocessor),
                        ", ",
                        stringify!($opcode1),
                        ", {v}, ",
                        stringify!($Crn),
                        ", ",
                        stringify!($Crm),
                        ", ",
                        stringify!($opcode2)
                    ),
                    v = in(reg) v
                );
            }
        }
    };
}
