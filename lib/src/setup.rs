use core::{fmt::Write, ops::DerefMut, panic::PanicInfo};

use crate::gpio::{Pin, PinFsel};
use crate::{
    interrupts,
    timer::delay_ms,
    uart::{get_uart_mut_checked, setup_uart, UartWriter},
};
use bcm2835_lpa::Peripherals;
use interrupts::disable_interrupts;

pub const SUPER_MODE: u32 = 0b10011;
pub const USER_MODE: u32 = 0b10000;
pub const STACK_ADDR: u32 = 0x8000000;

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

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    disable_interrupts();
    // If the uart is setup and not in use, then directly use it.
    // Otherwise, setup and write there.
    let construct_uart = || {
        let p14 = unsafe { Pin::<14, { PinFsel::Unset }>::forge() };
        let p15 = unsafe { Pin::<15, { PinFsel::Unset }>::forge() };
        setup_uart(p14, p15, unsafe { &mut Peripherals::steal() })
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
        let _ = w.write_fmt(format_args!("\n{}DONE!!!\n", info.message()));
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
