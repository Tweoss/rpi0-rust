use core::arch::{asm, global_asm};

use bcm2835_lpa::Peripherals;

use crate::{dsb, timer::timer_get_usec_raw};

// registers for ARM interrupt control
// bcm2835; p112   [starts at 0x2000b200]
const IRQ_BASE: u32 = 0x2000b200;
const IRQ_BASIC_PENDING: u32 = IRQ_BASE + 0x00; // 0x20;
const IRQ_PENDING_1: u32 = IRQ_BASE + 0x04; // 0x20;
const IRQ_PENDING_2: u32 = IRQ_BASE + 0x08; // 0x20;
const IRQ_FIQ_CONTROL: u32 = IRQ_BASE + 0x0c; // 0x20;
const IRQ_ENABLE_1: u32 = IRQ_BASE + 0x10; // 0x21;
const IRQ_ENABLE_2: u32 = IRQ_BASE + 0x14; // 0x21;
const IRQ_ENABLE_BASIC: u32 = IRQ_BASE + 0x18; // 0x21;
const IRQ_DISABLE_1: u32 = IRQ_BASE + 0x1c; // 0x21;
const IRQ_DISABLE_2: u32 = IRQ_BASE + 0x20; // 0x22;
const IRQ_DISABLE_BASIC: u32 = IRQ_BASE + 0x24; // 0x22;

const ARM_TIMER_IRQ: u32 = 1 << 0;
// registers for ARM timer
// bcm 14.2 p 196
const ARM_TIMER_BASE: u32 = 0x2000B400;
const ARM_TIMER_LOAD: u32 = ARM_TIMER_BASE + 0x00; // p196
const ARM_TIMER_VALUE: u32 = ARM_TIMER_BASE + 0x04; // read-only
const ARM_TIMER_CONTROL: u32 = ARM_TIMER_BASE + 0x08;
const ARM_TIMER_IRQ_CLEAR: u32 = ARM_TIMER_BASE + 0x0c;
// ...
const ARM_TIMER_RELOAD: u32 = ARM_TIMER_BASE + 0x18;
const ARM_TIMER_PREDIV: u32 = ARM_TIMER_BASE + 0x1c;
const ARM_TIMER_COUNTER: u32 = ARM_TIMER_BASE + 0x20;

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
    mrs r0,cpsr         @ move cpsr to r0
    bic r0,r0,#(1<<7)	@ clear 7th bit.
    msr cpsr_c,r0		@ move r0 back to PSR
    bx lr		        @ return.

@ disable them
.globl disable_interrupts
disable_interrupts:
    mrs r0,cpsr		       
    orr r0,r0,#(1<<7)	@ set 7th bit
    msr cpsr_c,r0
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
_software_interrupt_asm:      .word software_interrupt_asm
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
software_interrupt_asm:
    mov sp, {INT_STACK_ADDR}
    sub r0, lr, 4
    bl syscall_vector
prefetch_abort_asm:
    mov sp, {INT_STACK_ADDR}
    sub r0, lr, 4
    bl prefetch_abort_vector
data_abort_asm:
    mov sp, {INT_STACK_ADDR}
    sub r0, lr, 4
    bl data_abort_vector

"#,
    INT_STACK_ADDR = const INT_STACK_ADDR,
);

extern "C" {
    fn disable_interrupts();
    pub fn enable_interrupts();
}

/// one-time initialization of general purpose
/// interrupt state.
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
#[no_mangle]
extern "C" fn syscall_vector(pc: u32) {
    panic!("unexpected syscall: pc={}\n", pc);
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

// called by <interrupt-asm.S> on each interrupt.
#[no_mangle]
unsafe extern "C" fn interrupt_vector(_pc: u32) {
    let peripherals = Peripherals::steal();
    // we don't know what the client code was doing, so
    // start with a device barrier in case it was in
    // the middle of using a device (slow: you can
    // do tricks to remove this.)
    dsb();

    // get the interrupt source: typically if you have
    // one interrupt enabled, you'll have > 1, so have
    // to disambiguate what the source was.
    // let pending = (IRQ_BASIC_PENDING as *const u32).read_volatile();

    // if this isn't true, could be a GPU interrupt
    // (as discussed in Broadcom): just return.
    // [confusing, since we didn't enable!]
    // if (pending & ARM_TIMER_IRQ) == 0 {
    //     return;
    // }
    if !peripherals.LIC.basic_pending().read().timer().bit() {
        return;
    }

    // Clear the ARM Timer interrupt:
    // Q: what happens, exactly, if we delete?
    // peripherals.AUX.irq.
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
    // we don't need to do tempt entropy by getting cute.
    dsb();

    // Q: what happens (&why) if you uncomment the
    // print statement?
    // printk("In interrupt handler at time: %d\n", clk);
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
