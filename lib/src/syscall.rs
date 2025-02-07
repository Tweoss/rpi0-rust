use core::arch::asm;

use crate::setup::rpi_reboot;
use crate::{
    caches, cycle_counter,
    gpio::{Pin, PinFsel},
};

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

extern "C" fn umain() -> ! {
    println!("REACHED UMAIN");
    let v = syscall_hello(2);
    println!("syscall_hello {}", v);
    let v = syscall_error();
    println!("syscall_error {}", v);
    println!("FINISHED UMAIN\nDONE!!!");
    let mut p0 = unsafe { Pin::<0, { PinFsel::Unset }>::forge().into_output() };
    p0.write(true);
    let mut set_on = false;
    for _ in 0..4 {
        p0.write(set_on);
        set_on = !set_on;
        delay_ms(100);
    }
    rpi_reboot()
}

pub fn demo() {
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
    caches::enable();
    test_swi("normal", core::ptr::null());
    test_swi("slow", unsafe { _interrupt_table_slow.as_ptr() });
    test_swi("fast", unsafe { _interrupt_table_fast.as_ptr() });

    set_vector_table_base(core::ptr::null());
}

fn test_swi(label: &str, vector_table: *const u32) {
    println!("Testing {label} at addr: {:#010x}", vector_table as usize);
    set_vector_table_base(vector_table);
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

fn set_vector_table_base(new_table: *const u32) -> u32 {
    let old_address;
    unsafe { asm!("mrc p15, 0, {}, c12, c0, 0", out (reg) old_address ) }
    unsafe { asm!("mcr p15, 0, {}, c12, c0, 0", in (reg) (new_table as usize)) }
    old_address
}

//
// void notmain(void) {
//     // in <./interrupt-asm.S>
//     extern uint32_t _interrupt_vector_orig[];
//     extern uint32_t _interrupt_vector_slow[];
//     extern uint32_t _interrupt_vector_fast[];

//     // NOTE: we don't have to setup interrupts since we are only
//     // doing SWI calls.
//     vec_base_print("slow", _interrupt_vector_slow);
//     vec_base_print("fast", _interrupt_vector_fast);

//     test_swi("orig lab 5 vector(no cache)", _interrupt_vector_orig);
//     test_swi("better relocation (no cache)", _interrupt_vector_slow);
//     test_swi("fast relocation (no cache)", _interrupt_vector_fast);

//     output("\n");
//     trace("---------------------------------------------------------\n");
//     trace("------------------- turning on caching ------------------\n");
//     caches_enable();

//     test_swi("orig lab 5 (no cache)", _interrupt_vector_orig);
//     test_swi("better relocation (icache enabled)", _interrupt_vector_slow);
//     test_swi("fast relocation (icache enabled)", _interrupt_vector_fast);
// }

// void test_swi(const char *msg, void *v) {
//     trace("%s\n", msg);
//     set_base(v);

//     // we use alignment to try to get rid of prefetch buffer effects.
//     // should maybe include in the cycle count macro.
//     asm volatile (".align 4");
//     uint32_t s = cycle_cnt_read();
//         sys_plus1(1);
//     uint32_t t = cycle_cnt_read() - s;
//     trace("     single call took [%d] cycles\n", t);

//     asm volatile (".align 4");
//     s = cycle_cnt_read();
//         sys_plus1(1);
//     t = cycle_cnt_read() - s;
//     trace("     single call took [%d] cycles\n", t);

//     asm volatile (".align 4");
//     s = cycle_cnt_read();
//         sys_plus1(1);
//     t = cycle_cnt_read() - s;
//     trace("     single call took [%d] cycles\n", t);

//     asm volatile (".align 4");
//     s = cycle_cnt_read();
//     sys_plus1(1); // 1
//     sys_plus1(1); // 2
//     sys_plus1(1); // 3
//     sys_plus1(1); // 4
//     sys_plus1(1); // 5
//     sys_plus1(1); // 6
//     sys_plus1(1); // 7
//     sys_plus1(1); // 8
//     sys_plus1(1); // 9
//     sys_plus1(1); // 10
//     t = cycle_cnt_read() - s;
//     trace("     10 calls took [%d] cycles [%d] per call\n", t, t/10);
// }

// void set_base(void *v) {
//     vector_base_reset(v);

//     void *v_got = vector_base_get();
//     if(v_got != v)
//         panic("tried to set vector base <%p>: got <%p>\n", v,v_got);

//     // should be randomized of course.
//     enum { N = 20 };
//     for(unsigned i = 0; i < N; i++) {
//         uint32_t x = 0x12345678 + i;
//         uint32_t x_got = sys_plus1(x);
//         if((x + 1) != x_got)
//             panic("ERROR: sys_plus1(%x) == %x\n", x, x_got);
//     }
// }

// /*
//  * vector base address register:
//  *   arm1176.pdf:3-121 --- lets us control where the
//  *   exception jump table is!  makes it easy to switch
//  *   tables and also make exceptions faster.
//  *
//  * defines:
//  *  - vector_base_set
//  *  - vector_base_get
//  */
// // return the current value vector base is set to.
// static inline void *vector_base_get(void) {
//     todo("implement using inline assembly to get the vec base reg");
// }

// // check that not null and alignment is good.
// static inline int vector_base_chk(void *vector_base) {
//     if(!vector_base)
//         return 0;
//     todo("check alignment is correct: look at the instruction def!");
//     return 1;
// }

// // set vector base: must not have been set already.
// static inline void vector_base_set(void *vec) {
//     if(!vector_base_chk(vec))
//         panic("illegal vector base %p\n", vec);

//     void *v = vector_base_get();
//     // if already set to the same vector, just return.
//     if(v == vec)
//         return;

//     if(v)
//         panic("vector base register already set=%p\n", v);

//     todo("set vector base here.");

//     // double check that what we set is what we have.
//     v = vector_base_get();
//     if(v != vec)
//         panic("set vector=%p, but have %p\n", vec, v);
// }

// // set vector base to <vec> and return old value: could have
// // been previously set (i.e., non-null).
// static inline void *
// vector_base_reset(void *vec) {
//     void *old_vec = 0;

//     if(!vector_base_chk(vec))
//         panic("illegal vector base %p\n", vec);

//     todo("get old vector base, set new one\n");

//     // double check that what we set is what we have.
//     assert(vector_base_get() == vec);
//     return old_vec;
// }
