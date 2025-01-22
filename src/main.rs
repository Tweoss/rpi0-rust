#![no_std] // don't link the Rust standard library
#![no_main] // disable all Rust-level entry points
#![feature(adt_const_params)]
#![feature(start)]

use core::{arch::asm, fmt::Debug, panic::PanicInfo};

use bcm2835_lpa::{Peripherals, UART1};
use pi0_register::{Pin, PinFsel};

#[no_mangle] // don't mangle the name of this function
#[link_section = ".text.start"]
pub extern "C" fn _start() -> ! {
    let mut uart = unsafe { UART1::steal() };
    let mut peripherals = unsafe { Peripherals::steal() };
    let pins = unsafe { pi0_register::get_pins() };
    let (p0, pins): (Pin<0, { PinFsel::Unset }>, _) = pins.pluck();
    let (p14, pins): (Pin<14, { PinFsel::Unset }>, _) = pins.pluck();
    let (p15, pins): (Pin<15, { PinFsel::Unset }>, _) = pins.pluck();
    let _p14 = setup_uart(p14, p15, &mut peripherals);
    dsb();
    write_uart(&mut uart, "Hello!\n".as_bytes());

    let mut p0 = p0.into_output();
    p0.write(true);
    let mut set_on = false;
    loop {
        p0.write(set_on);
        set_on = !set_on;
        for _ in 0..200000 {
            unsafe { asm!("nop") }
        }
    }

    // let addr = 0x2020_0008 as *mut u32;
    // unsafe { *addr = (*addr & (!0x7_u32) << (6 * 3)) | 0x1 << (6 * 3) };

    // let set_addr = 0x2020_001C as *mut u32;
    // unsafe { *set_addr = !0x0_u32 };

    // let p = unsafe { Peripherals::steal() };
    // alt_blink(&p);
    // p.GPIO.gpfsel4().modify(|_, w| w.fsel47().output());

    //
    //     // let mut set_on = false;
    // loop {
    //     if set_on {
    //         unsafe { p.GPIO.gpset1().write_with_zero(|w| w.set47().set_bit()) };
    //     } else {
    //         unsafe {
    //             p.GPIO
    //                 .gpclr1()
    //                 .write_with_zero(|w| w.clr47().clear_bit_by_one())
    //         };
    //     }

    //     set_on = !set_on;
    //     for _ in 0..100000 {
    //         unsafe { asm!("nop") }
    //     }
    // }

    // #[allow(clippy::empty_loop)]
    // loop {}
}

const ASSUMED_CLOCK_RATE: usize = 250_000_000;
const DESIRED_BAUD_RATE: usize = 115_200;

fn setup_uart(
    p14: Pin<14, { PinFsel::Unset }>,
    p15: Pin<15, { PinFsel::Unset }>,
    peripherals: &mut Peripherals,
) -> Pin<14, { PinFsel::Alt5 }> {
    // Set pin 14 to TX. Needs to happen before enabling uart.
    let p = p14.into_alt5();
    // TODO: UART input
    // Enable uart.
    dsb();
    peripherals
        .AUX
        .enables()
        .modify(|_, w| w.uart_1().set_bit());
    dsb();

    let uart = &peripherals.UART1;
    // Clear the TX/RX fifos.
    // Disable uart TX/RX and flow control.
    uart.cntl().write(|w| unsafe { w.bits(0) });

    uart.ier().write(|w| unsafe { w.bits(0) });
    uart.iir()
        .modify(|_, w| w.tx_ready().set_bit().data_ready().set_bit());

    // TODO: disable interrupts
    // Set the baud rate.
    uart.baud().write(|w| unsafe {
        w.bits(
            (ASSUMED_CLOCK_RATE / 8 / DESIRED_BAUD_RATE - 1)
                .try_into()
                .unwrap(),
        )
    });
    uart.lcr().modify(|_, w| w.data_size()._8bit());
    uart.mcr().modify(|_, w| w.rts().clear_bit());
    // Enable TX/RX
    uart.cntl()
        .modify(|_, w| w.cts_enable().clear_bit().rts_enable().clear_bit());
    uart.cntl()
        .modify(|_, w| w.tx_enable().set_bit().rx_enable().set_bit());
    // TODO: enable interrupts
    p
}

fn write_uart(uart: &mut UART1, bytes: &[u8]) {
    for byte in bytes {
        // Wait until can write in a byte.
        while !uart.stat().read().tx_ready().bit_is_set() {
            unsafe { asm!("nop") }
        }
        uart.io().write(|w| unsafe { w.data().bits(*byte) });
    }
}

/// Device specific? barrier
fn dsb() {
    unsafe {
        asm!(
            "mcr p15, 0, {tmp}, c7, c10, 4",
            tmp = in(reg) 0,
        )
    }
}

/// This function is called on panic.
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

/// Demos loopback from gpio 9-> 10
/// blinks leds on gpio 26 and gpio 0 in an alteranting pattern
fn alt_blink(p: &Peripherals) -> ! {
    p.GPIO.gpfsel0().modify(|_, w| w.fsel9().output());
    p.GPIO.gpfsel1().modify(|_, w| w.fsel10().input());

    p.GPIO.gpfsel2().modify(|_, w| w.fsel26().output());
    p.GPIO.gpfsel0().modify(|_, w| w.fsel0().output());

    unsafe { p.GPIO.gpclr0().write_with_zero(|w| w.clr26().bit(true)) };

    let mut pin_9_on = false;
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
