use core::arch::asm;

use crate::setup::rpi_reboot;
use crate::{Pin, PinFsel};

use crate::{interrupts::run_user_code, println, timer::delay_ms};

pub fn syscall_vector(pc: u32) -> i32 {
    let instruction = unsafe { *(pc as *const u32) };
    assert!(
        (instruction >> 24) & 0b1111 == 0b1111,
        "must be a SWI instruction"
    );
    let syscall = instruction & 0xFFF;
    println!("got syscall: pc = {pc}, {syscall}\n");
    match syscall {
        0 => 0,
        1 => 0,
        2 => -1,
        _ => unimplemented!("Syscall {syscall} not yet implemented."),
    }
}

pub fn syscall_hello() -> i32 {
    let mut x;
    unsafe {
        asm!(
            "push {{lr}}
             swi 1
             pop {{lr}}
             mov {x}, r0",
             x = out(reg) x,
        )
    }
    x
}

pub fn syscall_error() -> i32 {
    let mut x;
    unsafe {
        asm!(
            "push {{lr}}
             swi 2
             pop {{lr}}
             mov {x}, r0",
             x = out(reg) x,
        )
    }
    x
}

extern "C" fn umain() -> ! {
    println!("REACHED UMAIN");
    let v = syscall_hello();
    println!("syscall_hello {}", v);
    let v = syscall_error();
    println!("syscall_error {}", v);
    println!("FINISHED UMAIN\nDONE!!!");
    let mut p0 = unsafe { Pin::<0, { PinFsel::Unset }>::forge().into_output() };
    p0.write(true);
    let mut set_on = false;
    for _ in 0..5 {
        p0.write(set_on);
        set_on = !set_on;
        delay_ms(100);
    }
    rpi_reboot()
}

pub fn demo() {
    run_user_code(umain);
}
