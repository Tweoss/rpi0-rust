#![no_std] // don't link the Rust standard library
#![no_main] // disable all Rust-level entry points
#![feature(start)]

use core::{arch::asm, panic::PanicInfo};

use bcm2835_lpa::Peripherals;

#[no_mangle] // don't mangle the name of this function
#[link_section = ".text.start"]
pub extern "C" fn _start() -> ! {
    // this function is the entry point, since the linker looks for a function
    // named `_start` by default

    // let addr = 0x2020_0008 as *mut u32;
    // unsafe { *addr = (*addr & (!0x7_u32) << (6 * 3)) | 0x1 << (6 * 3) };

    // let set_addr = 0x2020_001C as *mut u32;
    // unsafe { *set_addr = !0x0_u32 };

    let p = unsafe { Peripherals::steal() };

    p.GPIO.gpfsel0().modify(|_, w| w.fsel9().output());
    p.GPIO.gpfsel1().modify(|_, w| w.fsel10().input());

    p.GPIO.gpfsel2().modify(|_, w| w.fsel26().output());
    p.GPIO.gpfsel0().modify(|_, w| w.fsel0().output());

    unsafe { p.GPIO.gpclr0().write_with_zero(|w| w.clr26().bit(true)) };

    let mut pin_9_on = false;
    #[allow(clippy::empty_loop)]
    loop {
        let read_in = p.GPIO.gplev0().read().lev10().bit();

        if !read_in {
            unsafe { p.GPIO.gpset0().write_with_zero(|w| w.set0().set_bit()) };
        } else {
            unsafe {
                p.GPIO
                    .gpclr0()
                    .write_with_zero(|w| w.clr0().clear_bit_by_one())
            };
        }

        for _ in 0..100000 {
            unsafe { asm!("nop") }
        }
        pin_9_on = !pin_9_on;

        if pin_9_on {
            unsafe {
                p.GPIO
                    .gpset0()
                    .write_with_zero(|w| w.set9().bit(pin_9_on).set26().set_bit())
            };
            unsafe { p.GPIO.gpset0().write_with_zero(|w| w.set26().set_bit()) };
        } else {
            unsafe {
                p.GPIO
                    .gpclr0()
                    .write_with_zero(|w| w.clr9().clear_bit_by_one().clr26().clear_bit_by_one())
            };
            unsafe {
                p.GPIO
                    .gpclr0()
                    .write_with_zero(|w| w.clr26().clear_bit_by_one())
            };
        }
    }
}

/// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
