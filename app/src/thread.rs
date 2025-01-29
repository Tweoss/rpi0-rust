extern crate alloc;

use core::arch::{asm, global_asm};

use alloc::vec::Vec;
use heapless::Deque;

use crate::{
    dbg, println,
    setup::interrupts::{disable_interrupts, guard::InterruptGuard},
};

// Conclusion: things are pushed in ascending order
// sp always points one beyond
// pub extern "C" fn test_stack_pointer(arg1: u32, arg2: u32) {
//     let guard = InterruptGuard::new();
//     core::hint::black_box((arg1, arg2));
//     unsafe { asm!("push {{r1, r0}}") };
//     let (mut r0, mut r1): (u32, u32);
//     unsafe {
//         asm!("ldr {},[sp,#-8]
//               ldr {},[sp,#-4]",
//             out(reg) r0,
//             out(reg) r1)
//     };
//     dbg!(r0, r1);
//     println!("sup");
//     drop(guard);
// }
//
global_asm!(
    r#"
.globl thread_trampoline
thread_trampoline:
    @ set new stack pointer
    bl get_current_stack_pointer
    mov sp,r0
    pop {{r4, r5}}
    @ move argument from stored stack into r0
    mov r0,r4
    blx r5
    @ jump to exit function with exit code 0
    mov r0,#0
    b thread_exit

.globl call_saved_stack
call_saved_stack:
    push {{r4,r5}}       @ save prev r4,r5 before overwrite
    mov r4,r0            @ take the target pc from r0
    mov r5,lr            @ save link register before calling subroutine

    @ get dump register location
    bl get_register_dump_start_address
    mov r1,r4            @ move target pc back to safe range
    mov lr,r5            @ restore lr
    pop {{r4,r5}}        @ restore r4,r5

    @ dump all registers
    stmia r0,{{r4,r5,r6,r7,r8,r9,r10,r11,r12,sp,lr}}

    mov r5,r1            @ move target pc to safe range

    mov r0,pc            @ obtain the current program counter
    add r0,r0,#8         @ we want to reenter after the branch
    bl set_return_program_counter
    mov pc,r5            @ branch to target pc

    @ NOTE: REENTRANT HERE
    @ reload registers
    bl get_register_dump_start_address
    ldmia r0,{{r4,r5,r6,r7,r8,r9,r10,r11,r12,sp,lr}}

    bx lr                @ return to caller
    "#
);

const THREAD_MAX_STACK: usize = 1024 * 8 / 4;
const THREAD_COUNT: usize = 10;

struct ThreadState {
    queue: Deque<alloc::boxed::Box<Thread>, THREAD_COUNT>,
    current_thread: Option<alloc::boxed::Box<Thread>>,
    counter: u32,
    return_pc: Option<u32>,
    return_sp: Option<u32>,
    return_register_dump: Option<alloc::boxed::Box<[u32; 11]>>,
}

static mut THREAD_STATE: ThreadState = ThreadState {
    queue: Deque::new(),
    current_thread: None,
    counter: 0,
    return_pc: None,
    return_sp: None,
    return_register_dump: None,
};

struct Thread {
    id: u32,
    stack: Vec<u32>,
    stack_pointer: usize,
    program_counter: usize,
}

impl core::fmt::Debug for Thread {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!(
            "Thread {{
            id: {},
            stack_start: {:#010x},
            stack_length: {},
            stack_pointer: {},
            program_counter: {} }}",
            self.id,
            self.stack.as_ptr() as usize,
            self.stack.len(),
            self.stack_pointer,
            self.program_counter,
        ))
    }
}

/// Initializes thread system.
#[allow(static_mut_refs)]
pub fn start() {
    // TODO: have disable interrupts somehow return a mutable reference to all
    // static variables and have enable_interrupts take back the references?
    let state = unsafe { &mut THREAD_STATE };
    state.return_register_dump = Some(alloc::boxed::Box::new([0; 11]));

    loop {
        let guard = InterruptGuard::new();
        if let Some(next) = state.queue.pop_front() {
            println!("running thread {}", next.id);
            let pc = next.program_counter;
            println!("running next thread with pc {}", pc);
            state.current_thread = Some(next);
            drop(guard);
            unsafe { call_saved_stack(pc as u32) };
            println!("returned from call_saved_stack");
        } else {
            println!("no more threads to run");
            drop(guard);
            break;
        }
    }
}

#[allow(static_mut_refs)]
#[no_mangle]
pub extern "C" fn get_current_stack_pointer() -> u32 {
    println!("getting current stack pointer");
    unsafe { THREAD_STATE.current_thread.as_ref().unwrap().stack_pointer as u32 }
}
#[allow(static_mut_refs)]
#[no_mangle]
pub extern "C" fn set_current_stack_pointer(sp: u32) {
    println!("setting current stack pointer");
    unsafe { THREAD_STATE.current_thread.as_mut().unwrap().stack_pointer = sp as usize }
}

#[allow(static_mut_refs)]
#[no_mangle]
pub extern "C" fn set_return_program_counter(pc: u32) {
    dbg!(pc);
    unsafe { THREAD_STATE.return_pc = Some(pc) }
}
#[allow(static_mut_refs)]
#[no_mangle]
pub extern "C" fn get_register_dump_start_address() -> u32 {
    unsafe { (&mut THREAD_STATE.return_register_dump.as_mut().unwrap()).as_mut_ptr() as u32 }
}

#[allow(static_mut_refs)]
pub fn get_return_pc() -> u32 {
    unsafe { THREAD_STATE.return_pc.expect("should have had a return pc") as u32 }
}

extern "C" {
    pub fn thread_trampoline();
    pub fn call_saved_stack(branch_pc: u32) -> u32;
}

#[allow(static_mut_refs)]
pub fn fork(code: extern "C" fn(*mut u32), arg: *mut u32) {
    let mut stack = Vec::new();
    stack.resize(THREAD_MAX_STACK, 0);
    // should store code as "r4" and arg as "r5"
    // program_counter should point to trampoline
    let len = stack.len();
    stack[len - 1] = code as u32;
    stack[len - 2] = arg as u32;
    let state = unsafe { &mut THREAD_STATE };
    let id = state.counter;
    state.counter += 1;
    // After two pushes, stack_pointer will point at the lowest filled position.
    let stack_position = (&stack[len - 2]) as *const u32;

    let thread = Thread {
        id,
        stack,
        stack_pointer: stack_position as usize,
        program_counter: thread_trampoline as usize,
    };

    state
        .queue
        .push_back(alloc::boxed::Box::new(thread))
        .expect("too many threads");

    // TODO: push onto queue
    // 1. `rpi_fork` should write the address of trampoline
    //    `rpi_init_trampoline` to the `lr` offset in the newly
    //     created thread's stack (make sure you understand why!)  and the
    //     `code` and `arg` to some other register offsets (e.g., `r4` and
    //     `r5`) --- the exact offsets don't matter.

    // 2. Implement `rpi_init_trampoline` in `rpi-thread-asm.S` so that
    //    it loads arg` from the stack (from Step 1) into `r0`,
    //    loads `code` into another register that it then uses to
    //    do a branch and link.

    // 3. To handle missing `rpi_exit`: add a call to `rpi_exit` at the end
    //    of `rpi_init_trampoline`.

    // 4. To help debug problems: you can initially have the
    //    trampoline code you write (`rpi_init_trampoline`) initially just
    //    call out to C code to print out its values so you can sanity check
    //    that they make sense.
}

#[no_mangle]
pub extern "C" fn thread_exit(_exit_code: i32) -> ! {
    let return_pc = get_return_pc();
    // dbg!(return_pc);
    println!("return_pc = {}", return_pc);
    // Jump to start
    unsafe { asm!("bx {}", in(reg) return_pc) };
    unreachable!()
}

#[allow(static_mut_refs)]
pub fn thread_yield() {
    // Push self with new modified return address and jump to start
    unsafe { asm!("push {{r4, r5, r6, r7, r8, r9, r10, r11}}") };
    let return_pc = get_return_pc();
    let state = unsafe { &mut THREAD_STATE };
    let mut current_thread = state.current_thread.take().expect("should be in a thread");
    // TODO: should the boxes be pinned?
    let stored_pc_ptr = ((&mut current_thread.program_counter) as *mut usize) as usize;
    let current_thread_ptr = alloc::boxed::Box::<Thread>::into_raw(current_thread) as usize;

    #[no_mangle]
    pub extern "C" fn thread_yield_push_back(thread_ptr: usize) {
        let state = unsafe { &mut THREAD_STATE };
        state
            .queue
            .push_back(unsafe { alloc::boxed::Box::from_raw(thread_ptr as *mut Thread) })
            .expect("too many threads");
    }

    unsafe {
        asm!(
            "
            @ TODO: push current stack pointer
            mov r0,pc
            add r0,r0,#20 @ point to getting the stack pointer
            str r0,[{}]
            mov r0,{}
            mov r5,lr
            bl thread_yield_push_back
            mov lr,r5
            bx {}
            @ TODO: pop current stack pointer
            pop {{r4, r5, r6, r7, r8, r9, r10, r11}}
            ",
            in(reg) stored_pc_ptr,
            in(reg) current_thread_ptr,
            in(reg) return_pc
        )
    };
}

// // create a new thread that takes a single argument.
// typedef void (*rpi_code_t)(void *);

// rpi_thread_t *rpi_fork(rpi_code_t code, void *arg);

// // exit current thread: switch to the next runnable
// // thread, or exit the threads package.
// void rpi_exit(int exitcode);

// // yield the current thread.
// void rpi_yield(void);

// #define THREAD_MAXSTACK (1024 * 8/4)
// typedef struct rpi_thread {
//     // SUGGESTION:
//     //     check that this is within <stack> (see last field)
//     //     should never point outside.
//     uint32_t *saved_sp;

// 	struct rpi_thread *next;
// 	uint32_t tid;

//     // only used for part1: useful for testing without cswitch
//     void (*fn)(void *arg);
//     void *arg;          // this can serve as private data.

//     const char *annot;
//     // threads waiting on the current one to exit.
//     // struct rpi_thread *waiters;

// 	uint32_t stack[THREAD_MAXSTACK];
// } rpi_thread_t;
//
// _Static_assert(offsetof(rpi_thread_t, stack) % 8 == 0,
// "must be 8 byte aligned");

// // statically check that the register save area is at offset 0.
// _Static_assert(offsetof(rpi_thread_t, saved_sp) == 0,
// "stack save area must be at offset 0");

// // starts the thread system: only returns when there are
// // no more runnable threads.
// void rpi_thread_start(void);

// // get the pointer to the current thread.
// rpi_thread_t *rpi_cur_thread(void);

// // create a new thread that takes a single argument.
// typedef void (*rpi_code_t)(void *);

// rpi_thread_t *rpi_fork(rpi_code_t code, void *arg);

// // exit current thread: switch to the next runnable
// // thread, or exit the threads package.
// void rpi_exit(int exitcode);

// // yield the current thread.
// void rpi_yield(void);

// /***************************************************************
//  * internal routines: we put them here so you don't have to look
//  * for the prototype.
//  */
// // internal routine:
// //  - save the current register values into <old_save_area>
// //  - load the values in <new_save_area> into the registers
// //  reutrn to the caller (which will now be different!)
// void rpi_cswitch(uint32_t **old_sp_save, const uint32_t *new_sp);

// #if 0
// // returns the stack pointer (used for checking).
// const uint8_t *rpi_get_sp(void);

// // check that: the current thread's sp is within its stack.
// void rpi_stack_check(void);

// // do some internal consistency checks --- used for testing.
// void rpi_internal_check(void);

// // rpi_thread helpers
// static inline void *rpi_arg_get(rpi_thread_t *t) {
//     return t->arg;
// }
// static inline void rpi_arg_put(rpi_thread_t *t, void *arg) {
//     t->arg = arg;
// }
// #endif
// static inline unsigned rpi_tid(void) {
//     rpi_thread_t *t = rpi_cur_thread();
//     if(!t)
//         panic("rpi_threads not running\n");
//     return t->tid;
// }

// #endif
