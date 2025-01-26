pub mod interrupts;

use core::{
    arch::{asm, global_asm},
    fmt::Write,
    ops::DerefMut,
    panic::PanicInfo,
};

use bcm2835_lpa::Peripherals;
use interrupts::{disable_interrupts, interrupt_init, timer_init};
use pi0_register::{Pin, PinFsel};

use crate::{
    main,
    timer::delay_ms,
    uart::{get_uart_mut_checked, setup_uart, UartWriter},
};

const SUPER_MODE: u32 = 0b10011;
const STACK_ADDR: u32 = 0x8000000;

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

    _interrupt_table_end:   @ end of the table.
"#
, const SUPER_MODE, const STACK_ADDR);

#[no_mangle]
/// Taken from dawson engler's 140e staff code.
pub extern "C" fn rpi_reboot() -> ! {
    // uart_flush_tx();
    delay_ms(10);

    // is there a way to speed this up?
    let pm_rstc = 0x2010001c;
    let pm_wdog = 0x20100024;
    let pm_password = 0x5a000000;
    let pm_rstc_wrcfg_full_reset = 0x00000020;
    unsafe {
        (pm_wdog as *mut u32).write_volatile(pm_password | 1);
        (pm_rstc as *mut u32).write_volatile(pm_password | pm_rstc_wrcfg_full_reset);
    }
    #[allow(clippy::empty_loop)]
    loop {}
}

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
    timer_init(256, 0x10);
    // timer_init(16, 0x10);
    unsafe {
        interrupt_init();
    }

    // TODO: cycle count initialization
    // search for cycle_cnt_init.

    main()
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    disable_interrupts();
    // If the uart is setup and not in use, then directly use it.
    // Otherwise, setup and write there.
    let construct_uart = || {
        let pin = unsafe { Pin::<14, { PinFsel::Unset }>::forge() };
        setup_uart(pin, unsafe { &mut Peripherals::steal() })
    };

    fn write_panic(mut writer: impl DerefMut<Target = UartWriter>, info: &PanicInfo<'_>) {
        let mut w = writer.deref_mut();
        let _ = w.write_str("\npi panicked");
        if let Some(location) = info.location() {
            let _ = w.write_fmt(format_args!(
                " at {}:{}:{}",
                location.file(),
                location.line(),
                location.column()
            ));
        }
        let _ = w.write_fmt(format_args!("\n{}\n", info.message()));
    }
    if let Ok(mut reference) = unsafe { get_uart_mut_checked() } {
        if let Some(w) = reference.deref_mut() {
            write_panic(w, info);
            rpi_reboot();
        }
    }
    write_panic(&mut construct_uart(), info);
    rpi_reboot();
}
