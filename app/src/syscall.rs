use core::arch::asm;

use crate::println;

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
