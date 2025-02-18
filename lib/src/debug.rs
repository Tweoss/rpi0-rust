//! # Debug hardware functionality.
//!
use core::cell::RefCell;

use alloc::{boxed::Box, vec::Vec};
use bitfield::bitfield;
use critical_section::Mutex;

// TODO: grab all registers when switching into prefetch abort
//       have routine to switch between threads of control

use crate::{
    cp_asm_get, cp_asm_set, dbg, interrupts::run_user_code, prefetch_flush, println,
    setup::rpi_reboot, steal_print, steal_println,
};

pub fn data_abort_vector(pc: u32) {
    if let WatchpointStatus::Enabled { .. } = get_watchpoint_status() {
        steal_println!("pc = {:04x}", pc);
        set_watchpoint_status(WatchpointStatus::Disabled);
        return;
    }

    panic!("unexpected data abort: pc={}\n", pc);
}

#[derive(Debug, Clone)]
pub struct Registers {
    pub pc: u32,
    pub lr: u32,
    pub sp: u32,
    pub r: [u32; 13],
}

static PREVIOUS_REGISTERS: Mutex<RefCell<Option<Registers>>> = Mutex::new(RefCell::new(None));

static THREADS: Mutex<RefCell<Vec<Thread>>> = Mutex::new(RefCell::new(Vec::new()));

struct Thread {
    function_pointer: usize,
    registers: Registers,
    stack: Vec<u32>,
    finished: bool,
    switch_at: Option<usize>,
    current_count: usize,
}

// TODO: Safe because we don't have multiple threads running at same time?
unsafe impl Send for Thread {}

impl Thread {
    fn from_fn(f: extern "C" fn()) -> Self {
        let pc = f as *const u32 as usize;
        let mut stack = Vec::new();
        stack.resize(1_000_usize, 0);

        Self {
            function_pointer: pc,
            registers: Registers {
                lr: thread_finished as *const () as u32,
                pc: pc as u32,
                sp: unsafe { (&stack[0] as *const u32).offset(stack.len() as isize) } as u32,
                r: [0_u32; 13],
            },
            finished: false,
            stack,
            switch_at: None,
            current_count: 0,
        }
    }
}

// static A_INSTRUCTION_COUNT: Mutex<RefCell<usize>> = Mutex::new(RefCell::new(0));
// static B_INSTRUCTION_COUNT: Mutex<RefCell<usize>> = Mutex::new(RefCell::new(0));

// If we're running interleaving, we stash the original registers here.
static RUNNING_INTERLEAVING: Mutex<RefCell<Option<(usize, Registers)>>> =
    Mutex::new(RefCell::new(None));

pub fn prefetch_abort_vector(pc: u32, registers: Registers) -> Registers {
    let cs = unsafe { critical_section::CriticalSection::new() };

    if RUNNING_INTERLEAVING.borrow_ref_mut(cs).is_none() {
        if pc == run_interleaving as *const () as u32 {
            steal_println!("STARTING INTERLAVINg");
            let threads = THREADS.borrow_ref_mut(cs);
            let thread = threads
                .last()
                .expect("tried to start threading without a thread");
            let index = threads.len() - 1;
            *RUNNING_INTERLEAVING.borrow_ref_mut(cs) = Some((index, registers));
            return thread.registers.clone();
        }
        // steal_println!("not running interleaving, continuing");
        set_breakpoint_address(pc);
        return registers;
    }

    // We are threading.
    // TODO: handle interleaving
    // Disable single stepping and also switch between threads.
    if pc == thread_finished as *const () as u32 {
        steal_println!("thread finished");
        let mut threads = THREADS.borrow_ref_mut(cs);
        let mut running = RUNNING_INTERLEAVING.borrow_ref_mut(cs);
        let inner = running.as_mut().unwrap();
        threads.remove(inner.0);
        if let Some(next_thread) = threads.last() {
            let index = threads.len() - 1;
            inner.0 = index;
            return next_thread.registers.clone();
        }

        set_breakpoint_status(BreakpointStatus::Disabled);
        return running.take().unwrap().1;
    }
    if let BreakpointStatus::Enabled { matching: false } = get_breakpoint_status() {
        // Update the pc for single stepping.
        set_breakpoint_address(pc);
        return registers.clone();
    }

    return registers;

    let mut prev = PREVIOUS_REGISTERS.borrow_ref_mut(cs);

    // Have entered a.
    // let a_switch_addr = (a as *mut ()) as usize + *A_INSTRUCTION_COUNT.borrow_ref(cs) * 8;

    // if pc as usize == a_switch_addr {
    //     // if A_INSTRUCTION_COUNT == 0 {

    //     // }
    //     steal_println!("running b");
    //     b();
    // }

    if let Some(p) = &*prev {
        let r = registers;
        steal_print!("updates: ");
        if r.pc != p.pc {
            steal_print!("pc = {}, ", r.pc);
        }
        if r.lr != p.lr {
            steal_print!("lr = {}, ", r.lr);
        }
        if r.sp != p.sp {
            steal_print!("sp = {}, ", r.sp);
        }
        steal_println!("");
    }
    // Disable single stepping and also switch between threads.
    if pc == spawn as *const () as u32 {
        dbg!(registers.r[0]);
        steal_println!("pushing back");
        // set_breakpoint_status(BreakpointStatus::Disabled);
        return registers.clone();
    }

    // steal_println!("registers: {:?}", registers);
    steal_println!("step pc: {:04x}", pc);
    *prev = Some(registers.clone());
    // Disable single stepping.
    if pc == disable_single_stepping as *const () as u32 {
        steal_println!("disabling single stepping");
        set_breakpoint_status(BreakpointStatus::Disabled);
        return registers.clone();
    }
    if let BreakpointStatus::Enabled { matching: false } = get_breakpoint_status() {
        // Update the pc for single stepping.
        set_breakpoint_address(pc);
        return registers.clone();
    }
    panic!("unexpected prefetch abort: pc={}\n", pc);
}

bitfield! {
    /// Debug status and control register.
    struct DSCR(u32);
    get_monitor_debug_mode_enabled, set_monitor_debug_mode_enabled: 15;
    get_halting_debug_mode_selected, set_halting_debug_mode_selected: 14;
    get_interrupts_disabled, set_interrupts_disabled: 11;
}

cp_asm_get!(dscr_get, p14, 0, c0, c1, 0);
cp_asm_set!(dscr_set, p14, 0, c0, c1, 0);
cp_asm_set!(wvr0_set, p14, 0, c0, c0, 6);

bitfield! {
    /// Watchpoint control register.
    struct WCR(u32);
    get_linking, set_linking: 20;
    get_watchpoint_matches, set_watchpoint_matches: 15,14;
    get_byte_address_select, set_byte_address_select: 8,5;
    get_load_stores, set_load_stores: 4,3;
    get_access_condition, set_access_condition: 2,1;
    get_enabled, set_enabled: 0;
}

cp_asm_get!(wcr0_get, p14, 0, c0, c0, 7);
cp_asm_set!(wcr0_set, p14, 0, c0, c0, 7);

#[derive(Debug)]
pub enum WatchpointStatus {
    Enabled { load: bool, store: bool },
    Disabled,
}

pub fn get_watchpoint_status() -> WatchpointStatus {
    let wcr = WCR(wcr0_get());
    match (wcr.get_enabled(), wcr.get_load_stores()) {
        (true, v) => WatchpointStatus::Enabled {
            load: v >= 2,
            store: (v & 1) == 1,
        },
        (false, _) => WatchpointStatus::Disabled,
    }
}

pub fn set_watchpoint_status(status: WatchpointStatus) {
    let mut wcr = WCR(wcr0_get());
    match status {
        WatchpointStatus::Enabled { load, store } => {
            wcr.set_load_stores(if load { 0b10 } else { 0b00 } + if store { 1 } else { 0 });
            wcr.set_enabled(true);
        }
        WatchpointStatus::Disabled => wcr.set_enabled(false),
    }
    wcr0_set(wcr.0);
    prefetch_flush();
}

pub fn set_watchpoint_address(addr: u32) {
    wvr0_set(addr);
}

cp_asm_set!(bvr0_set, p14, 0, c0, c0, 4);

bitfield! {
    /// Breakpoint control register.
    struct BCR(u32);
    // Note context id and mismatching cannot both be true.
    get_mismatching, set_mismatching: 22;
    get_context_id, set_context_id: 21;
    get_linking, set_linking: 20;
    get_breakpoint_matches, set_breakpoint_matches: 15,14;
    get_byte_address_select, set_byte_address_select: 8,5;
    get_access_condition, set_access_condition: 2,1;
    get_enabled, set_enabled: 0;
}

cp_asm_get!(bcr0_get, p14, 0, c0, c0, 5);
cp_asm_set!(bcr0_set, p14, 0, c0, c0, 5);

#[derive(Debug)]
pub enum BreakpointStatus {
    Disabled,
    Enabled { matching: bool },
}

pub fn get_breakpoint_status() -> BreakpointStatus {
    let bcr = BCR(bcr0_get());
    match (bcr.get_enabled(), bcr.get_mismatching()) {
        (true, v) => BreakpointStatus::Enabled { matching: !v },
        (false, _) => BreakpointStatus::Disabled,
    }
}

pub fn set_breakpoint_status(status: BreakpointStatus) {
    let mut bcr = BCR(bcr0_get());
    match status {
        BreakpointStatus::Enabled { matching } => {
            bcr.set_mismatching(!matching);
            bcr.set_enabled(true);
        }
        BreakpointStatus::Disabled => bcr.set_enabled(false),
    }
    bcr0_set(bcr.0);
    prefetch_flush();
}

pub fn set_breakpoint_address(addr: u32) {
    bvr0_set(addr);
    // TODO: might need prefetch fluhs.
}

#[inline(never)]
pub fn disable_single_stepping() {
    core::hint::black_box(0);
}

pub fn setup() {
    // Enabled the debug processor.
    let mut dscr = DSCR(dscr_get());
    dscr.set_monitor_debug_mode_enabled(true);
    dscr.set_halting_debug_mode_selected(false);
    dscr.set_interrupts_disabled(true);
    dscr_set(dscr.0);

    wvr0_set(0);
    let mut wcr = WCR(0);
    wcr.set_linking(false);
    // All bytes.
    wcr.set_byte_address_select(0b1111);
    // Either load or store.
    wcr.set_load_stores(0b11);
    // Privileged mode or not.
    wcr.set_access_condition(0b11);
    // wcr.set_enabled(true);
    wcr0_set(wcr.0);

    bvr0_set(0);
    let mut bcr = BCR(0);
    // Disable linking, no context ID matching, break on instruction modified
    // virtual address (IMVA) mismatch
    bcr.set_linking(false);
    bcr.set_context_id(false);
    bcr.set_mismatching(false);
    // Match in secure and non-secure mode.
    bcr.set_breakpoint_matches(0b00);
    // All bytes.
    bcr.set_byte_address_select(0b1111);
    // Privileged mode or not.
    bcr.set_access_condition(0b11);
    bcr0_set(bcr.0);

    crate::prefetch_flush();
}

// TODO:
// implement try_lock
//
// implement pre-emptive thread for single stepping. capture r0-r14.
// this will only work on user mode because if processor is in privileged mode,
// mismatch won't trigger interrupts (13-33 of arm1176.pdf)
//
// implement fork, (exec with mode), exit, waitpid, yield. only need to save r4-14+cpsr
//
// want to be able to yield, calling a function and switching mode
// need to dump CPSR too apparently
//
// 1. have dumping register, basically just copy
// 2.

static mut SHARED_STATE: usize = 0;

#[inline(never)]
fn spawn(f: extern "C" fn() -> ()) {
    // TODO: push back onto threads
    critical_section::with(|cs| {
        THREADS.borrow_ref_mut(cs).push(Thread::from_fn(f));
    });
    core::hint::black_box(f);
}

#[inline(never)]
fn run_interleaving() {
    core::hint::black_box(0);
}

#[inline(never)]
fn thread_finished() {
    println!("thread finished");
    core::hint::black_box(0);
}

extern "C" fn a() {
    println!("sup from a");
    println!("a should now exit");
    unsafe { SHARED_STATE += 1 };
    // thread_finished((a as *mut ()) as usize);
}

extern "C" fn b() {
    println!("sup from b");
    unsafe { SHARED_STATE += 1 };
    dbg!(unsafe { SHARED_STATE });
}

fn check_state_success() -> bool {
    unsafe { SHARED_STATE == 2 }
}

extern "C" fn umain() -> ! {
    // TODO: allow spawn to set current offset (still exit if hit thread_finished though)
    //       have run_interleaving return whether or not finished
    spawn(a);
    spawn(b);

    run_interleaving();

    // let a = 1;
    // let mut b = a + a;
    // b = b * b << a + 23;
    // for i in 0..1 {
    //     core::hint::black_box(fibonacci(core::hint::black_box(i)));
    // }
    // core::hint::black_box(b);

    disable_single_stepping();
    println!("UMAIN DONE!!!");
    rpi_reboot()
}

pub fn demo() {
    // a();
    // b();

    set_watchpoint_status(WatchpointStatus::Disabled);
    set_breakpoint_address(0);
    set_breakpoint_status(BreakpointStatus::Enabled { matching: false });

    run_user_code(umain);
    // set_breakpoint_address(0);
    // set_breakpoint_status(BreakpointStatus::Enabled { matching: false });

    a();

    // for
    // Handler runs b after
    // a();
}
