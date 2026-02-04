use core::arch::asm;

use crate::coprocessor::{
    set_breakpoint_address, set_breakpoint_status, set_vector_table_base, BreakpointStatus, CPSR,
};
use crate::gpio::Output;
use crate::setup::rpi_reboot;
use crate::{cycle_counter, gpio::Pin};

use crate::{interrupts::run_user_code, println, timer::delay_ms};

pub fn syscall_vector(pc: u32) -> i32 {
    let instruction = unsafe { *(pc as *const u32) };
    assert!(
        (instruction >> 24) & 0b1111 == 0b1111,
        "must be a SWI instruction"
    );
    let syscall = instruction & 0xFFF;
    // println!("got syscall: pc = {pc}, {syscall}\n");
    match syscall {
        0 => 0,
        1 => 0,
        2 => -1,
        3 => {
            set_breakpoint_address(0);
            set_breakpoint_status(BreakpointStatus::Enabled { matching: false });
            0
        }
        _ => unimplemented!("Syscall {syscall} not yet implemented."),
    }
}

pub extern "C" fn syscall_hello(v: u32) -> u32 {
    let mut x;
    unsafe {
        asm!(
            "push {{lr}}
             mov r0,{v}
             swi 1
             pop {{lr}}
             mov {x}, r0",
             v = in(reg) v,
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

pub extern "C" fn syscall_enable_breakpoint_mismatch(v: u32) -> u32 {
    let mut x;
    unsafe {
        asm!(
            "push {{lr}}
             mov r0,{v}
             swi 3
             pop {{lr}}
             mov {x}, r0",
             v = in(reg) v,
             x = out(reg) x,
        )
    }
    x
}

extern "C" fn umain() -> ! {
    println!("REACHED UMAIN");
    let v = syscall_hello(2);
    println!("syscall_hello {}", v);
    let v = syscall_error();
    println!("syscall_error {}", v);
    println!("FINISHED UMAIN\nDONE!!!");
    let mut p0: Pin<0, Output> = unsafe { todo!() };
    // let mut p0: Pin<0,  Output > =
    //     unsafe { Pin::<0,  Unset >::forge().into_output() };
    p0.write(true);
    let mut set_on = false;
    for _ in 0..4 {
        p0.write(set_on);
        set_on = !set_on;
        delay_ms(100);
    }
    rpi_reboot()
}

pub fn demo() -> ! {
    test_interrupt_speed();
    run_user_code(umain);
}

/// Use different interrupt vector table locations + branches.
#[allow(static_mut_refs)]
pub fn test_interrupt_speed() {
    extern "C" {
        static mut _interrupt_table_slow: [u32; 7];
        static mut _interrupt_table_fast: [u32; 7];
    }
    test_swi("normal", core::ptr::null());
    test_swi("slow", unsafe { _interrupt_table_slow.as_ptr() });
    test_swi("fast", unsafe { _interrupt_table_fast.as_ptr() });
    let mut cpsr = CPSR::read();
    cpsr.set_instruction_cache_enabled(true);
    cpsr.set_branch_prediction_enabled(true);
    CPSR::write(cpsr);

    test_swi("normal", core::ptr::null());
    test_swi("slow", unsafe { _interrupt_table_slow.as_ptr() });
    test_swi("fast", unsafe { _interrupt_table_fast.as_ptr() });

    set_vector_table_base((core::ptr::null::<u32>() as usize) as u32);
}

fn test_swi(label: &str, vector_table: *const u32) {
    println!("Testing {label} at addr: {:#010x}", vector_table as usize);
    set_vector_table_base(vector_table as u32);
    let start = cycle_counter::read();
    syscall_hello(1);
    let end = cycle_counter::read();
    println!("\tsingle call took \t{:10}", end - start);

    let start = cycle_counter::read();
    syscall_hello(1);
    let end = cycle_counter::read();
    println!("\tsingle call took \t{:10}", end - start);

    let start = cycle_counter::read();
    syscall_hello(1);
    let end = cycle_counter::read();
    println!("\tsingle call took \t{:10}", end - start);

    let start = cycle_counter::read();
    syscall_hello(1);
    syscall_hello(1);
    syscall_hello(1);
    syscall_hello(1);
    syscall_hello(1);
    syscall_hello(1);
    syscall_hello(1);
    syscall_hello(1);
    syscall_hello(1);
    syscall_hello(1);
    let end = cycle_counter::read();
    println!("\t10 calls took \t\t{:10}", (end - start) / 10);

    // let mut x = 100;
    // for _ in 0..100 {
    //     x = (16807_u32.wrapping_mul(x)) % 2_147_483_647;
    //     assert!(syscall_hello(x) == x.wrapping_add(1));
    //     // assert!(syscall_hello(x) == x.wrapping_add(1));
    // }
}
