use core::arch::asm;

use pi0_lib::interrupts::interrupt_init;

use crate::main;

#[no_mangle]
pub unsafe extern "C" fn rsstart() {
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

    main()
}
