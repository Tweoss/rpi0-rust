use core::arch::{asm, global_asm};
use pi0_lib::{
    interrupts::interrupt_init,
    setup::rpi_reboot,
    setup::{STACK_ADDR, SUPER_MODE},
};

use crate::main;

global_asm!(r#"
.section ".text.start"
.globl _start
_start:
    @ force the mode to be SUPER.
    mov r0,  {}
    orr r0,r0,#(1<<7)    @ disable interrupts.
    msr cpsr, r0

    @ prefetch flush
    mov r1, #0;
    mcr p15, 0, r1, c7, c5, 4

    mov sp, {}          @ initialize stack pointer
    mov fp, #0          @ clear frame pointer reg.  don't think needed.
    bl rsstart          @ we could jump right to rsstart (notmain)
    @ bl _cstart        @ call our code to do initialization.
    bl rpi_reboot     @ if they return just reboot.

    @ _interrupt_table_end:   @ end of the table.
"#
, const SUPER_MODE, const STACK_ADDR);

#[no_mangle]
pub unsafe extern "C" fn rsstart() -> ! {
    // Safety: I *believe* this is sufficient to prevent compiler reorderings.
    // https://stackoverflow.com/questions/72823056/how-to-build-a-barrier-by-rust-asm
    asm!("");
    extern "C" {
        static mut __bss_start__: u8;
        static mut __bss_end__: u8;
    }
    // Not sure if this is sound.
    // Was unable to observe nonzeroed BSS before, so saw no change.
    let count = &raw const __bss_end__ as usize - &raw const __bss_start__ as usize;
    core::ptr::write_bytes(&raw mut __bss_start__, 0, count);
    asm!("");

    //     // now setup timer interrupts.
    //     //  - Q: if you change 0x100?
    //     //  - Q: if you change 16?
    // interrupts::timer_init(1, 0x100);
    // interrupts::timer_init(16, 0x10);
    unsafe {
        interrupt_init();
    }

    // TODO: cycle count initialization
    // search for cycle_cnt_init.

    main();
    rpi_reboot();
}
