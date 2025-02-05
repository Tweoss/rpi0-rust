#![no_std]
#![no_main]

use core::{
    arch::{asm, global_asm},
    time::Duration,
};

use bcm2835_lpa::Peripherals;
use bootloader_shared::{
    BASE, CRC_ALGORITHM, INSTALLER_PROG_INFO, INSTALLER_SUCCESS, PI_ERROR, PI_GET_CODE,
    PI_GET_PROG_INFO, PI_SUCCESS,
};
use pi0_lib::{
    setup::{rpi_reboot, STACK_ADDR, SUPER_MODE},
    timer,
    uart::{read_uart, read_uart_u32_timeout, setup_uart, store_uart, write_uart_u32},
    Pin, PinFsel,
};

const BOOTLOADER_LOCATION: u32 = 0x200000;

global_asm!(r#"
.section ".text.start"
.globl _start
_start:
    mov sp,#0x08000000
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

@ give ourselves space to load the bootloaded program
.space {LOCATION}-0x8024,0
"#
, const SUPER_MODE, const STACK_ADDR, LOCATION = const BOOTLOADER_LOCATION);

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

    main();

    rpi_reboot();
}

fn main() {
    let p0 = unsafe { Pin::<0, { PinFsel::Unset }>::forge() };
    let mut p0 = p0.into_output();
    let uart = setup_uart(
        unsafe { Pin::<14, { PinFsel::Unset }>::forge() },
        unsafe { Pin::<15, { PinFsel::Unset }>::forge() },
        unsafe { &mut Peripherals::steal() },
    );
    store_uart(uart);

    if let Err(()) = load() {
        write_uart_u32(PI_ERROR);
        p0.write(false);
        timer::delay_ms(500);
        p0.write(true);
        timer::delay_ms(500);
        p0.write(false);
        rpi_reboot();
    }
    p0.write(false);

    // Jump to the loaded code!
    unsafe { asm!("mov pc,{}", const BASE) };
}

fn load() -> Result<(), ()> {
    // Wait for message indicating transmission while sending program info req.
    loop {
        write_uart_u32(PI_GET_PROG_INFO);
        if let Ok(v) = read_uart_u32_timeout(Duration::from_millis(300)) {
            if v != INSTALLER_PROG_INFO {
                return Err(());
            }
            break;
        }
    }

    // Receive message length.
    let program_length = read_uart_u32_timeout(Duration::from_millis(10))?;

    // If there's not enough space, error.
    if BASE + program_length >= BOOTLOADER_LOCATION {
        return Err(());
    }

    let checksum = read_uart_u32_timeout(Duration::from_millis(10))?;

    // Request code and have other side validate checksum.
    write_uart_u32(PI_GET_CODE);
    write_uart_u32(checksum);

    // Receive and copy in code.
    let mut start = timer::timer_get_usec();
    let mut index = BASE;
    let mut digest = CRC_ALGORITHM.digest_with_initial(0);
    loop {
        let mut buf = [0; 8];
        let buf = read_uart(&mut buf);
        if buf.is_empty() {
            // If we time out waiting for a single byte, return.
            if Duration::from_micros((timer::timer_get_usec() - start) as u64)
                > Duration::from_millis(10)
            {
                return Err(());
            }
        } else {
            start = timer::timer_get_usec();
        }
        let dest = unsafe { core::slice::from_raw_parts_mut(index as *mut u8, buf.len()) };
        dest.copy_from_slice(buf);
        digest.update(buf);

        index += buf.len() as u32;

        if index - BASE == program_length {
            break;
        }
    }

    // Verify checksum.
    let calculated = digest.finalize();
    if calculated != checksum {
        return Err(());
    }

    write_uart_u32(PI_SUCCESS);
    if read_uart_u32_timeout(Duration::from_millis(10))? != INSTALLER_SUCCESS {
        return Err(());
    }

    Ok(())
}
