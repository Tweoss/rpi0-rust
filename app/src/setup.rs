use bcm2835_lpa::Peripherals;
use core::arch::{asm, global_asm};
use pi0_lib::{
    cycle_counter, get_pins,
    gpio::{Pin, PinFsel},
    interrupts::{gpio_interrupts_init, interrupt_init, timer_initialized},
    setup::{__bss_end__, __bss_start__, rpi_reboot, STACK_ADDR, SUPER_MODE},
    uart::{setup_uart, store_uart},
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
    // Not sure if this is sound.
    // Was unable to observe nonzeroed BSS before, so saw no change.
    let count = (&raw const __bss_end__).byte_offset_from(&raw const __bss_start__);

    for index in 0..count {
        // Use assembly instead of a slice copy because rust/LLVM believes that
        // is guaranteed to be undefined => unreachable after.
        let dest = (&raw mut __bss_start__).byte_offset(index * (size_of::<u32>() as isize));
        let source = 0_u32;
        asm!("str {}, [{}]", in(reg) source, in(reg) dest);
    }
    asm!("");
    interrupt_init();

    //     // now setup timer interrupts.
    //     //  - Q: if you change 0x100?
    //     //  - Q: if you change 16?
    assert!(!timer_initialized());
    // // interrupts::timer_init(1, 0x100);
    // interrupts::timer_init(16, 0x1000);
    // assert!(timer_initialized());
    gpio_interrupts_init();

    pi0_lib::debug::setup();

    cycle_counter::init();

    let mut peripherals = unsafe { Peripherals::steal() };
    let pins = unsafe { get_pins() };
    let (p14, pins): (Pin<14, { PinFsel::Unset }>, _) = pins.pluck();
    let (p15, _pins): (Pin<15, { PinFsel::Unset }>, _) = pins.pluck();
    let w = setup_uart(p14, p15, &mut peripherals);
    store_uart(w);

    pi0_lib::virtual_memory::setup();

    main();
    rpi_reboot();
}
