extern crate alloc;

use core::arch::{asm, global_asm};

use alloc::vec::Vec;
use heapless::Deque;

use crate::{interrupts::guard::InterruptGuard, println};

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

.globl call_and_save_stack
call_and_save_stack:
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

.globl yield_save_context
yield_save_context:
    push {{r4, r5, r6, r7, r8, r9, r10, r11, lr}}
    mov r4,r0                    @ put target pc in safe register 

    mov r0,sp                    @ save stack pointer after pushes
    mov r1,pc                    @ grab current pc
    add r1,r1,#8                @ point to reloading the stack pointer

    @ save sp, pc in current thread's state and move thread to run queue
    bl thread_yield_save_sp_move_to_queue 

    @ mov r3,sp                    @ save stack pointer after pushes
    @ bl set_current_stack_pointer
 
    @ mov r3,pc                    @ grab current pc
    @ add r3,r3,#12                @ point to reloading the stack pointer
    @ str r3,[r0]                  @ save modified pc to thread struct
    @ mov r0,r1                    @ push thread struct onto run queue
    @ bl thread_yield_push_back

    bx r4                        @ yield to logic that handles run queue 

    @ REENTRY HERE
    bl get_current_stack_pointer @ grab stack pointer from thread struct
    mov sp,r0                    @ restore stack and all other registers
    pop {{r4, r5, r6, r7, r8, r9, r10, r11, lr}}
    bx lr                        @ return to caller
    "#
);

const THREAD_MAX_STACK: usize = 1024 * 8 / 4;
const THREAD_COUNT: usize = 10;

struct ThreadState {
    queue: Deque<alloc::boxed::Box<Thread>, THREAD_COUNT>,
    current_thread: Option<alloc::boxed::Box<Thread>>,
    counter: u32,
    return_pc: Option<u32>,
    return_register_dump: Option<alloc::boxed::Box<[u32; 11]>>,
}

static mut THREAD_STATE: ThreadState = ThreadState {
    queue: Deque::new(),
    current_thread: None,
    counter: 0,
    return_pc: None,
    return_register_dump: None,
};

pub struct Thread {
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
            let pc = next.program_counter;
            println!(
                "running thread {} with pc {}",
                next.id, next.program_counter
            );
            state.current_thread = Some(next);
            drop(guard);
            unsafe { call_and_save_stack(pc as u32) };
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
    unsafe { THREAD_STATE.current_thread.as_ref().unwrap().stack_pointer as u32 }
}

#[allow(static_mut_refs)]
#[no_mangle]
pub extern "C" fn set_return_program_counter(pc: u32) {
    unsafe { THREAD_STATE.return_pc = Some(pc) }
}
#[allow(static_mut_refs)]
#[no_mangle]
pub extern "C" fn get_register_dump_start_address() -> u32 {
    unsafe { (&mut THREAD_STATE.return_register_dump.as_mut().unwrap()).as_mut_ptr() as u32 }
}
#[allow(static_mut_refs)]
#[no_mangle]
pub extern "C" fn thread_yield_save_sp_move_to_queue(new_sp: u32, new_pc: u32) {
    let state = unsafe { &mut THREAD_STATE };
    let mut current_thread = state.current_thread.take().expect("should be in a thread");
    current_thread.stack_pointer = new_sp as usize;
    current_thread.program_counter = new_pc as usize;
    state
        .queue
        .push_back(current_thread)
        .expect("too many threads");
}
#[allow(static_mut_refs)]
#[no_mangle]
pub extern "C" fn thread_yield_push_back(thread_ptr: usize) {
    let state = unsafe { &mut THREAD_STATE };
    state
        .queue
        .push_back(unsafe { alloc::boxed::Box::from_raw(thread_ptr as *mut Thread) })
        .expect("too many threads");
}
#[allow(static_mut_refs)]
pub fn get_return_pc() -> u32 {
    unsafe { THREAD_STATE.return_pc.expect("should have had a return pc") as u32 }
}

extern "C" {
    pub fn thread_trampoline();
    pub fn call_and_save_stack(branch_pc: u32) -> u32;
    pub fn yield_save_context(return_pc: u32);
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
}

#[no_mangle]
pub extern "C" fn thread_exit(_exit_code: i32) -> ! {
    let return_pc = get_return_pc();
    // Jump to start
    unsafe { asm!("bx {}", in(reg) return_pc) };
    unreachable!()
}

pub fn thread_yield() {
    // Push self with new modified return address and jump to start
    let return_pc = get_return_pc();
    // TODO: should the boxes be pinned?
    unsafe { yield_save_context(return_pc) };
}

#[allow(static_mut_refs)]
pub fn read_current_thread<T>(f: impl FnOnce(&Thread) -> T) -> Option<T> {
    let state = unsafe { &THREAD_STATE };
    if let Some(current) = &state.current_thread {
        Some(f(current))
    } else {
        None
    }
}

pub fn demo() {
    static mut ARGS: [u32; 3] = [42, 27, 1];
    extern "C" fn thread(arg: *mut u32) {
        let id = read_current_thread(|t| t.id).unwrap();

        println!("thread {id} has arg: {}", unsafe { *arg });
        for i in 0..3 {
            println!("thread {id} yields {i}");
            thread_yield();
        }
        if id == 1 {
            println!("forking");
            fork(thread, &mut unsafe { ARGS }[2]);
        }
        println!("thread {id} exits now");
        thread_exit(1);
    }
    fork(thread, &mut unsafe { ARGS }[0]);
    fork(thread, &mut unsafe { ARGS }[1]);
    start();
}
