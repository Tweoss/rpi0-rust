use core::{
    arch::{asm, global_asm},
    cell::RefCell,
};

use alloc::{boxed::Box, vec::Vec};
use bcm2835_lpa::Peripherals;
use critical_section::Mutex;

use crate::{dsb, gpio::Pin, println, timer::timer_get_usec_raw};

// registers for ARM interrupt control
// bcm2835; p112   [starts at 0x2000b200]
const IRQ_BASE: u32 = 0x2000b200;
// const IRQ_BASIC_PENDING: u32 = IRQ_BASE + 0x00; // 0x20;
// const IRQ_PENDING_1: u32 = IRQ_BASE + 0x04; // 0x20;
// const IRQ_PENDING_2: u32 = IRQ_BASE + 0x08; // 0x20;
// const IRQ_FIQ_CONTROL: u32 = IRQ_BASE + 0x0c; // 0x20;
// const IRQ_ENABLE_1: u32 = IRQ_BASE + 0x10; // 0x21;
// const IRQ_ENABLE_2: u32 = IRQ_BASE + 0x14; // 0x21;
const IRQ_ENABLE_BASIC: u32 = IRQ_BASE + 0x18; // 0x21;

// const IRQ_DISABLE_1: u32 = IRQ_BASE + 0x1c; // 0x21;
// const IRQ_DISABLE_2: u32 = IRQ_BASE + 0x20; // 0x22;
// const IRQ_DISABLE_BASIC: u32 = IRQ_BASE + 0x24; // 0x22;

const ARM_TIMER_IRQ: u32 = 1 << 0;
// registers for ARM timer
// bcm 14.2 p 196
const ARM_TIMER_BASE: u32 = 0x2000B400;
const ARM_TIMER_LOAD: u32 = ARM_TIMER_BASE + 0x00; // p196
                                                   // const ARM_TIMER_VALUE: u32 = ARM_TIMER_BASE + 0x04; // read-only
const ARM_TIMER_CONTROL: u32 = ARM_TIMER_BASE + 0x08;
const ARM_TIMER_IRQ_CLEAR: u32 = ARM_TIMER_BASE + 0x0c;
// ...
// const ARM_TIMER_RELOAD: u32 = ARM_TIMER_BASE + 0x18;
// const ARM_TIMER_PREDIV: u32 = ARM_TIMER_BASE + 0x1c;
// const ARM_TIMER_COUNTER: u32 = ARM_TIMER_BASE + 0x20;

const INT_STACK_ADDR: u32 = 0x9000000;

global_asm!(
    r#"
/*
 * Enable/disable interrupts.
 *
 * CPSR = current program status register
 *        upper bits are different carry flags.
 *        lower 8:
 *           7 6 5 4 3 2 1 0
 *          +-+-+-+---------+
 *          |I|F|T|   Mode  |
 *          +-+-+-+---------+
 *
 *  I : disables IRQ when = 1.
 *  F : disables FIQ when = 1.
 *  T : = 0 indicates ARM execution, 
 *      = 1 is thumb execution.  
 *  Mode = current mode.
 */

@ enable system interrupts by modifying cpsr.
@    note: should make a version that returns 
@    previous state.
@ <.globl> makes name visible to other files
.globl enable_interrupts  
enable_interrupts:
    mrs r0,cpsr         @ move cpsr to r1
    bic r1,r0,#(1<<7)	@ clear 7th bit.
    @ TODO: check if can race the value of r0 if
    @ interrupted right here. some stack overflow answers
    @ suggest my solution but don't say if it can race
    msr cpsr_c,r1		@ move r1 back to PSR
    @ return whether or not interrupts were enabled
    lsr r0,r0,#7
    and r0,r0,#1
    bx lr		        @ return.

@ disable them. returns whether or not they were previously enabled
.globl disable_interrupts
disable_interrupts:
    mrs r0,cpsr		       
    orr r1,r0,#(1<<7)	@ set 7th bit
    msr cpsr_c,r1
    lsr r0,r0,#7
    and r0,r0,#1
    bx lr

@ disable them. returns whether or not they were previously enabled
.globl interrupts_enabled
interrupts_enabled:
    mrs r0,cpsr		       
    lsr r0,r0,#7
    and r0,r0,#1
    bx lr

@ the interrupt table that we copy to 0x0.
@   - start = <_interrupt_table>
@   - end = <_interrupt_table_end>
@   - look at the disassembly
@
@ note: *it must be position independent since
@ we copy it!*
.globl _interrupt_table
.globl _interrupt_table_end
_interrupt_table:
  @ Q: why can we copy these ldr jumps and have
  @ them work the same?
  ldr pc, _reset_asm
  ldr pc, _undefined_instruction_asm
  ldr pc, _software_interrupt_asm
  ldr pc, _prefetch_abort_asm
  ldr pc, _data_abort_asm
  ldr pc, _reset_asm
  ldr pc, _interrupt_asm
fast_interrupt_asm:
  sub   lr, lr, #4 @First instr of FIQ handler
  push  {{lr}}
  push  {{r0-r12}}
  mov   r0, lr              @ Pass old pc
  bl    fast_interrupt_vector    @ C function
  pop   {{r0-r12}}
  ldm   sp!, {{pc}}^

_reset_asm:                   .word reset_asm
_undefined_instruction_asm:   .word undefined_instruction_asm
@ temporarily use plus1 instead
@ _software_interrupt_asm:      .word software_interrupt_asm
_software_interrupt_asm:      .word sys_plus1_handler
_prefetch_abort_asm:          .word prefetch_abort_asm
_data_abort_asm:              .word data_abort_asm
_interrupt_asm:               .word interrupt_asm
_interrupt_table_end:   @ end of the table.

@ only handler that should run since we 
@ only enable general interrupts
interrupt_asm:
  @ NOTE:
  @  - each mode has its own <sp> that persists when
  @    we switch out of the mode (i.e., will be the same
  @    when switch back).
  @  - <INT_STACK_ADDR> is a physical address we reserve 
  @   for exception stacks today.  we don't do recursive
  @   exception/interupts so one stack is enough.
  mov sp, {INT_STACK_ADDR}   
  sub   lr, lr, #4

  push  {{r0-r12,lr}}         @ XXX: pushing too many 
                            @ registers: only need caller
                            @ saved.

  mov   r0, lr              @ Pass old pc as arg 0
  bl    interrupt_vector    @ C function: expects C 
                            @ calling conventions.

  pop   {{r0-r12,lr}} 	    @ pop integer registers
                            @ this MUST MATCH the push.
                            @ very common mistake.

  @ return from interrupt handler: will re-enable general ints.
  @ Q: what happens if you do "mov" instead?
  @ Q: what other instructions could we use?
  movs    pc, lr        @ 1: moves <spsr> into <cpsr> 
                        @ 2. moves <lr> into the <pc> of that
                        @    mode.

@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@
@ currently we don't use any of these, so just panic and
@ and halt.  should not occur!

reset_asm:
    @ Q: if we delete this instruction what happens?
    mov sp, {INT_STACK_ADDR}   
    @ Q: is <4> correct?  how to tell?
    sub   r0, lr, #4
    bl    reset_vector

@ note: we compile this <.S> file with gcc 
@ so that we can use C macro's to clean up.
@ note the use of semi-colons!
@ #define unexpected(fn, offset)      \
@     mov sp, {INT_STACK_ADDR};        \
@     sub   r0, lr, #(offset);        \
@     bl    fn

@ Q: what are the right offsets for the following?
undefined_instruction_asm:
    mov sp, {INT_STACK_ADDR}
    sub r0, lr, 4
    bl undefined_instruction_vector


@ runs at system level (p.a2-5): assumes we have a sp
@
@ you're going to call:
@    int syscall_vector(unsigned pc, uint32_t r0) 
@
@   1 save regs as with interrupt vector
@   2 figure out the lr offset you need to correct.
@   3 mov the original r0 into r1 (so it's the second
@     parameter to <syscall_vector>)
@   4 mov the pointer to the syscall inst into r0
@     (so its the first parameter to <syscall_vector>)
@   5 call <syscall_vector>
@   6 restore regs: must be identical to what got pushed 
@     at step (1)
@   - return from the exception (look at interrupt_asm)
software_interrupt_asm:
    mov sp, {INT_STACK_ADDR}  @ TODO: check necessary
    @ sub   lr, lr, #4

    push  {{r4-r12,lr}}     @ XXX: pushing too many 
                            @ registers: only need caller
                            @ saved.

    sub   r0, lr, 4         @ Pass old pc (go back by 4) as arg 0 
    bl    syscall_vector    @ C function: expects C 
                            @ calling conventions.

    pop   {{r4-r12,lr}} 	@ pop integer registers
                            @ this MUST MATCH the push.
                            @ very common mistake.

    @ return from interrupt handler: will re-enable general ints.
    @ Q: what happens if you do "mov" instead?
    @ Q: what other instructions could we use?
    movs    pc, lr          @ 1: moves <spsr> into <cpsr> 
                            @ 2. moves <lr> into the <pc> of that mode.
    @ mov sp, {INT_STACK_ADDR}
    @ sub r0, lr, 4
    @ bl syscall_vector
prefetch_abort_asm:
    mov sp, {INT_STACK_ADDR}
    sub r0, lr, 4
    bl prefetch_abort_vector
data_abort_asm:
    mov sp, {INT_STACK_ADDR}
    sub r0, lr, 4
    bl data_abort_vector

@
@ utility routine:
@   1. switch to user mode, 
@   2. set setting <sp> to <stack>
@   3. jump to the address in <pc> 
@ 
@ recall:
@   - pc is passed in r0
@   - stack passed in r1
@
@ look at notes/mode-bugs for examples on what to do
@    - change modes using the cps instruction.
@
@ does not return
run_user_code_asm:
    mov r2,  {USER_MODE}
    orr r2,r2,#(1<<7)    @ disable interrupts.
    msr cpsr, r2
    @ prefetch flush
    mov r3, #0;
    mcr p15, 0, r3, c7, c5, 4
    @ set new stack pointer from r1
    mov sp, r1
    @ jump to the program counter in r0
    mov pc, r0
    @ user main should never return

.globl sys_plus1_handler
sys_plus1_handler:
    add r0, r0, #1
    movs pc, lr

.align 5
.globl _interrupt_table_slow
_interrupt_table_slow:
    ldr pc, =reset_asm
    ldr pc, =undefined_instruction_asm
    ldr pc, =sys_plus1_handler
    ldr pc, =prefetch_abort_asm
    ldr pc, =data_abort_asm
    ldr pc, =reset_asm
    ldr pc, =interrupt_asm

.align 5
.globl _interrupt_table_fast
_interrupt_table_fast:
    ldr pc, =reset_asm
    ldr pc, =undefined_instruction_asm
    @ since we assume _interrupt_vector is not relocated, we can use a branch
    @ instead of a trampoline. (branches must be +/- 32MB)
    b sys_plus1_handler
    ldr pc, =prefetch_abort_asm
    ldr pc, =data_abort_asm
    ldr pc, =reset_asm
    ldr pc, =interrupt_asm

"#,
    INT_STACK_ADDR = const INT_STACK_ADDR,
    USER_MODE = const super::setup::USER_MODE,
);

mod asm {
    extern "C" {
        /// Returns 0 if previously enabled, 0 else.
        pub fn enable_interrupts() -> u32;
        /// Returns 0 if previously enabled, 0 else.
        pub fn disable_interrupts() -> u32;
        /// Returns 0 if previously enabled, 0 else.
        pub fn interrupts_enabled() -> u32;
        pub fn run_user_code_asm(f: extern "C" fn() -> !, sp: *const u32) -> !;
    }
}

/// Returns true if the interrupts were previously enabled, and false if they
/// were previously disabled.
pub fn enable_interrupts() -> bool {
    unsafe { asm::enable_interrupts() == 0 }
}
/// Returns true if the interrupts were previously enabled, and false if they
/// were previously disabled.
pub fn disable_interrupts() -> bool {
    unsafe { asm::disable_interrupts() == 0 }
}
/// Returns true if the interrupts were previously enabled, and false if they
/// were previously disabled.
pub fn interrupts_enabled() -> bool {
    unsafe { asm::interrupts_enabled() == 0 }
}

#[allow(static_mut_refs)]
pub fn run_user_code(f: extern "C" fn() -> !) {
    // Use the top of the stack because it grows down.
    let stack = (unsafe { USER_STACK.last().unwrap() }) as *const u32;
    assert!(!stack.is_null());
    assert!(stack.is_aligned());
    unsafe { asm::run_user_code_asm(f, stack) }
}

/// one-time initialization of general purpose interrupt state.
pub unsafe fn interrupt_init() {
    // printk("about to install interrupt handlers\n");
    //
    // turn off global interrupts.
    disable_interrupts();
    // TODO: why no dsb here?

    // put interrupt flags in known state by disabling
    // all interrupt sources (1 = disable).
    //  BCM2835 manual, section 7.5 , 112
    (Peripherals::steal()).AUX.irq().write(|w| w.bits(u32::MAX));
    // (IRQ_DISABLE_1 as *mut u32).write_volatile(0xffffffff);
    dsb();

    // Copy interrupt vector table and FIQ handler.
    //   - <_interrupt_table>: start address
    //   - <_interrupt_table_end>: end address
    // these are defined as labels in the global assembly.

    extern "C" {
        static mut _interrupt_table: u32;
        static mut _interrupt_table_end: u32;
    }
    let len = (&raw const _interrupt_table_end).offset_from(&raw const _interrupt_table) as u32;
    let table = unsafe { core::slice::from_raw_parts_mut(&raw mut _interrupt_table, len as usize) };
    asm!("");
    for (index, source) in table.iter().enumerate() {
        // Use assembly instead of a slice copy because rust/LLVM believes that
        // is guaranteed to be undefined => unreachable after.
        let dest = index * size_of::<u32>();
        asm!("str {}, [{}]", in(reg) *source, in(reg) dest);
    }
    asm!("");
}

#[no_mangle]
extern "C" fn fast_interrupt_vector(pc: u32) {
    panic!("unexpected fast interrupt: pc={}\n", pc);
}

const USER_STACK_SIZE: usize = 1024 * 64 * 2;
static mut USER_STACK: [u32; USER_STACK_SIZE] = [0; USER_STACK_SIZE];

#[no_mangle]
extern "C" fn syscall_vector(pc: u32) -> i32 {
    crate::syscall::syscall_vector(pc)
}
#[no_mangle]
extern "C" fn reset_vector(pc: u32) {
    panic!("unexpected reset: pc={}\n", pc);
}
#[no_mangle]
extern "C" fn undefined_instruction_vector(pc: u32) {
    panic!("unexpected undef-inst: pc={}\n", pc);
}
#[no_mangle]
extern "C" fn prefetch_abort_vector(pc: u32) {
    panic!("unexpected prefetch abort: pc={}\n", pc);
}
#[no_mangle]
extern "C" fn data_abort_vector(pc: u32) {
    panic!("unexpected data abort: pc={}\n", pc);
}

static mut CNT: u32 = 0;
static mut PERIOD: u32 = 0;
static mut PERIOD_SUM: u32 = 0;
static mut LAST_CLK: u32 = 0;

pub unsafe fn get_cnt() -> u32 {
    unsafe { CNT }
}
pub unsafe fn get_period() -> u32 {
    unsafe { PERIOD }
}

static INTERRUPT_HANDLERS: Mutex<RefCell<Vec<Option<Box<dyn FnMut(u32) + Send + Sync>>>>> =
    Mutex::new(RefCell::new(alloc::vec![]));
static UNUSED_HANDLER_SLOTS: Mutex<RefCell<Vec<usize>>> = Mutex::new(RefCell::new(alloc::vec![]));

// called by <interrupt-asm.S> on each interrupt.
#[no_mangle]
unsafe extern "C" fn interrupt_vector(pc: u32) {
    let peripherals = Peripherals::steal();
    // we don't know what the client code was doing, so
    // start with a device barrier in case it was in
    // the middle of using a device (slow: you can
    // do tricks to remove this.)
    dsb();
    // Safe because we are inside an interrupt, which means the hardware disabled
    // interrupts for us.
    let cs = unsafe { critical_section::CriticalSection::new() };
    println!("got interrupt");

    for handler in INTERRUPT_HANDLERS.borrow_ref_mut(cs).iter_mut() {
        let Some(handler) = handler else { continue };
        handler(pc);
    }

    // get the interrupt source: typically if you have
    // one interrupt enabled, you'll have > 1, so have
    // to disambiguate what the source was.
    // let pending = (IRQ_BASIC_PENDING as *const u32).read_volatile();

    // if this isn't true, could be a GPU interrupt
    // (as discussed in Broadcom): just return.
    // [confusing, since we didn't enable!]
    if !peripherals.LIC.basic_pending().read().timer().bit() {
        return;
    }
    // TODO: move timer handling into handler.

    // Clear the ARM Timer interrupt:
    // Q: what happens, exactly, if we delete?
    (ARM_TIMER_IRQ_CLEAR as *mut u32).write_volatile(1);

    // note: <staff-src/timer.c:timer_get_usec_raw()>
    // accesses the timer device, which is different
    // than the interrupt subsystem.  so we need
    // a dev_barrier() before.
    dsb();

    CNT += 1;

    // compute time since the last interrupt.
    let clk = timer_get_usec_raw();
    PERIOD = if LAST_CLK != 0 { clk - LAST_CLK } else { 0 };
    PERIOD_SUM += PERIOD;
    LAST_CLK = clk;

    // we don't know what the client was doing,
    // so, again, do a barrier at the end of the
    // interrupt handler.
    //
    // NOTE: i think you can make an argument that
    // this barrier is superflous given that the timer
    // access is a read that we wait for, but for the
    // moment we live in simplicity: there's enough
    // bad stuff that can happen with interrupts that
    // we don't need to tempt entropy by getting cute.
    (ARM_TIMER_IRQ_CLEAR as *mut u32).write_volatile(1);
    dsb();
}

pub fn timer_clear() {
    unsafe { (ARM_TIMER_IRQ_CLEAR as *mut u32).write_volatile(1) };
}

// // initialize timer interrupts.
// // <prescale> can be 1, 16, 256. see the timer value.
// // NOTE: a better interface = specify the timer period.
// // worth doing as an extension!
// static
pub unsafe fn timer_init(prescale: u32, ncycles: u32) {
    //**************************************************
    // now that we are sure the global interrupt state is
    // in a known, off state, setup and enable
    // timer interrupts.
    // printk("setting up timer interrupts\n");

    // assume we don't know what was happening before.
    dsb();

    // bcm p 116
    // write a 1 to enable the timer inerrupt ,
    // "all other bits are unaffected"
    // TODO: use peripherals crate
    // unsafe { Peripherals::steal() }.AUX.irq()
    //
    (IRQ_ENABLE_BASIC as *mut u32).write_volatile(ARM_TIMER_IRQ);

    // dev barrier b/c the ARM timer is a different device
    // than the interrupt controller.
    dsb();

    // Timer frequency = Clk/256 * Load
    //   - so smaller <Load> = = more frequent.
    (ARM_TIMER_LOAD as *mut u32).write_volatile(ncycles);

    // bcm p 197
    // note errata!  not a 23 bit.
    const ARM_TIMER_CTRL_32BIT: u32 = 1 << 1;
    const ARM_TIMER_CTRL_PRESCALE_1: u32 = 0 << 2;
    const ARM_TIMER_CTRL_PRESCALE_16: u32 = 1 << 2;
    const ARM_TIMER_CTRL_PRESCALE_256: u32 = 2 << 2;
    const ARM_TIMER_CTRL_INT_ENABLE: u32 = 1 << 5;
    const ARM_TIMER_CTRL_ENABLE: u32 = 1 << 7;

    let v = match prescale {
        1 => ARM_TIMER_CTRL_PRESCALE_1,
        16 => ARM_TIMER_CTRL_PRESCALE_16,
        256 => ARM_TIMER_CTRL_PRESCALE_256,
        _ => panic!("illegal prescale = {}", prescale),
    };
    // Q: if you change prescale?
    (ARM_TIMER_CONTROL as *mut u32).write_volatile(
        ARM_TIMER_CTRL_32BIT | ARM_TIMER_CTRL_ENABLE | ARM_TIMER_CTRL_INT_ENABLE | v,
    );

    // done modifying timer: do a dev barrier since
    // we don't know what device gets used next.
    dsb();
}

pub fn timer_initialized() -> bool {
    (unsafe { (IRQ_ENABLE_BASIC as *mut u32).read_volatile() } & ARM_TIMER_IRQ) != 0
}

pub unsafe fn gpio_interrupts_init() {
    dsb();
    let p = unsafe { bcm2835_lpa::Peripherals::steal() };
    p.LIC.enable_2().write(|w| {
        w.gpio_0()
            .set_bit()
            .gpio_1()
            .set_bit()
            .gpio_2()
            .set_bit()
            .gpio_3()
            .set_bit()
    });
    dsb();
}

/// Returns an index to remove the handler.
pub fn register_interrupt_handler(handler: Box<impl Fn(u32) + Sync + Send + 'static>) -> usize {
    critical_section::with(|cs| {
        let mut handlers = INTERRUPT_HANDLERS.borrow_ref_mut(cs);
        if let Some(index) = UNUSED_HANDLER_SLOTS.borrow_ref_mut(cs).pop() {
            assert!(handlers[index].is_none());
            handlers[index] = Some(handler);
            return index;
        }
        let index = handlers.len();
        handlers.push(Some(handler));
        index
    })
}

/// Returns an index to remove the handler.
pub fn remove_interrupt_handler(index: usize) {
    critical_section::with(|cs| {
        let mut handlers = INTERRUPT_HANDLERS.borrow_ref_mut(cs);
        handlers[index] = None;
        UNUSED_HANDLER_SLOTS.borrow_ref_mut(cs).push(index);
    })
}
