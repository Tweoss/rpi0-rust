//! # Debug hardware functionality.
//!
//! Includes single stepping thread functionality.
use core::cell::RefCell;

use alloc::{collections::btree_map::BTreeMap, rc::Rc, vec::Vec};
use critical_section::Mutex;

use crate::{
    coprocessor::{
        bvr0_set, dfsr_get, far_get, get_breakpoint_status, get_watchpoint_status, ifar_get,
        set_breakpoint_address, set_breakpoint_status, set_watchpoint_status, wvr0_set,
        BreakpointStatus, WatchpointStatus, BCR0, DSCR, WCR0,
    },
    dbg,
    interrupts::run_user_code,
    println,
    setup::rpi_reboot,
    steal_println,
    syscall::syscall_enable_breakpoint_mismatch,
};

pub const CRC_ALGORITHM: crc::Crc<u32> = crc::Crc::<u32>::new(&crc::CRC_32_BZIP2);

pub fn data_abort_vector(pc: u32) {
    if let WatchpointStatus::Enabled { .. } = get_watchpoint_status() {
        steal_println!("pc = {:04x}", pc);
        set_watchpoint_status(WatchpointStatus::Disabled);
        return;
    }

    println!(
        "faulted at pc={:#010x}, ifar={:#010x}, dfsr={:#010x}, far={:#010x}",
        pc,
        ifar_get(),
        dfsr_get(),
        far_get()
    );
    if far_get() == 0xa77acc0_u32 {
        println!("allowing");
        crate::virtual_memory::allow_segment_illegal_access();
        return;
    }
    panic!("unexpected data abort: pc={:#010x}\n", pc);
}

#[derive(Debug, Clone)]
pub struct Registers {
    pub pc: u32,
    pub lr: u32,
    pub sp: u32,
    pub r: [u32; 13],
}

static THREADS: Mutex<RefCell<BTreeMap<usize, Thread>>> = Mutex::new(RefCell::new(BTreeMap::new()));
static FINISHED_THREADS: Mutex<RefCell<Vec<Thread>>> = Mutex::new(RefCell::new(Vec::new()));

struct Thread {
    id: usize,
    function_pointer: usize,
    registers: Registers,
    stack: Vec<u32>,
    finished: bool,
    current_count: usize,
    crc: crc::Digest<'static, u32>,
}

impl core::fmt::Debug for Thread {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Thread")
            .field("id", &self.id)
            .field("function_pointer", &self.function_pointer)
            .field("registers", &self.registers)
            .field("stack", &self.stack.as_ptr())
            .field("finished", &self.finished)
            .field("current_count", &self.current_count)
            .finish()
    }
}

// TODO: Safe because we don't have multiple threads running at same time?
unsafe impl Send for Thread {}

static THREAD_COUNT: Mutex<RefCell<usize>> = Mutex::new(RefCell::new(0));
impl Thread {
    fn from_fn(f: extern "C" fn()) -> Self {
        let pc = f as *const u32 as usize;
        let mut stack = Vec::new();
        stack.resize(1_000_usize, 0);
        let thread_id = critical_section::with(|cs| {
            let mut tc = THREAD_COUNT.borrow_ref_mut(cs);
            let out = *tc;
            *tc += 1;
            out
        });

        Self {
            id: thread_id,
            function_pointer: pc,
            registers: Registers {
                lr: thread_finished as *const () as u32,
                pc: pc as u32,
                sp: unsafe { (&stack[0] as *const u32).offset(stack.len() as isize) } as u32,
                r: [0_u32; 13],
            },
            finished: false,
            stack,
            current_count: 0,
            crc: CRC_ALGORITHM.digest_with_initial(0),
        }
    }

    /// Reset the thread to a function pointer.
    ///
    /// Reuses the stack and thread id.
    fn reset(mut self, f: extern "C" fn()) -> Self {
        let pc = f as *const u32 as usize;
        self.function_pointer = pc;
        self.registers.lr = thread_finished as *const () as u32;
        self.registers.pc = pc as u32;
        self.registers.sp =
            unsafe { (&self.stack[0] as *const u32).offset(self.stack.len() as isize) } as u32;
        // TODO: could reset registers, but meh
        self.finished = false; // necessary?
        self.current_count = 0;
        self.crc = CRC_ALGORITHM.digest_with_initial(0);

        self
    }
}

// If we're running interleaving, we stash the original (main thread) registers here.
static RUNNING_INTERLEAVING: Mutex<RefCell<Option<(usize, Registers)>>> =
    Mutex::new(RefCell::new(None));

static SCHEDULER: Mutex<RefCell<Option<Scheduler>>> = Mutex::new(RefCell::new(None));
struct Scheduler(Rc<dyn Fn(usize, &BTreeMap<usize, Thread>) -> usize>);
unsafe impl Send for Scheduler {}

pub fn prefetch_abort_vector(pc: u32, registers: Registers) -> Registers {
    let get_breakpoint_status = get_breakpoint_status();
    if far_get() == 0xa77acc0_u32 {
        println!("allowing prefetch abort");
        crate::virtual_memory::allow_segment_illegal_access();
        return registers;
    }

    if let BreakpointStatus::Disabled = get_breakpoint_status {
        panic!("unexpected prefetch abort: pc={}\n", pc);
    }

    let cs = unsafe { critical_section::CriticalSection::new() };

    let mut running_option = RUNNING_INTERLEAVING.borrow_ref_mut(cs);
    let Some(running) = running_option.as_mut() else {
        if pc == trigger_interleaving as *const () as u32 {
            let threads = THREADS.borrow_ref_mut(cs);
            let thread = threads
                .first_key_value()
                .expect("tried to start threading without a thread")
                .1;
            *running_option = Some((thread.id, registers));
            return thread.registers.clone();
        }
        // steal_println!("not running interleaving, continuing");
        set_breakpoint_address(pc);
        return registers;
    };

    // We are threading.
    let mut threads = THREADS.borrow_ref_mut(cs);
    let mut borrow_ref_mut = SCHEDULER.borrow_ref_mut(cs);
    let scheduler = borrow_ref_mut.as_mut().unwrap();
    let current = threads.get_mut(&running.0).unwrap();
    current.registers = registers.clone();
    current.current_count += 1;
    current.crc.update(&current.registers.pc.to_le_bytes());
    current.crc.update(&current.registers.lr.to_le_bytes());
    current.crc.update(&current.registers.sp.to_le_bytes());
    for r in current.registers.r {
        current.crc.update(&r.to_le_bytes());
    }

    // Disable single stepping and also switch between threads.
    if pc == thread_finished as *const () as u32 {
        let mut done = threads.remove(&running.0).unwrap();
        done.finished = true;
        // Let the main thread reuse the thread allocation.
        let mut finished = FINISHED_THREADS.borrow_ref_mut(cs);
        finished.push(done);
        if threads.is_empty() {
            // Return from threading.
            set_breakpoint_status(BreakpointStatus::Disabled);
            return running_option.take().unwrap().1;
        } else {
            running.0 = *threads.first_key_value().unwrap().0;
            let decision = scheduler.0(running.0, &threads);
            if !threads.contains_key(&decision) {
                panic!(
                    "scheduler decision out of range, {:?}, {}",
                    threads, decision
                );
            }
            running.0 = decision;
            set_breakpoint_address(threads[&running.0].registers.pc);
            return threads[&decision].registers.clone();
        }
    }

    // If running in mismatch mode.
    if let BreakpointStatus::Enabled { matching: false } = get_breakpoint_status {
        let decision = scheduler.0(running.0, &threads);
        if !threads.contains_key(&decision) {
            dbg!(&threads, decision);
            panic!("scheduler decision out of range");
        }
        running.0 = decision;
        set_breakpoint_address(threads[&running.0].registers.pc);
        return threads[&running.0].registers.clone();
    }

    return registers;
}
#[inline(never)]
pub fn disable_single_stepping() {
    core::hint::black_box(0);
}

pub fn setup() {
    // Enabled the debug processor.
    let mut dscr = DSCR::read();
    dscr.set_monitor_debug_mode_enabled(true);
    dscr.set_halting_debug_mode_selected(false);
    dscr.set_interrupts_disabled(true);
    DSCR::write(dscr);

    wvr0_set(0);

    let mut wcr = WCR0(0);
    wcr.set_linking(false);
    // All bytes.
    wcr.set_byte_address_select(0b1111);
    // Either load or store.
    wcr.set_load_stores(0b11);
    // Privileged mode or not.
    wcr.set_access_condition(0b11);
    WCR0::write(wcr);

    bvr0_set(0);

    let mut bcr = BCR0::read();
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
    BCR0::write(bcr);

    crate::prefetch_flush();
}

// TODO:
// implement try_lock

static mut SHARED_STATE: usize = 0;

#[inline(never)]
fn spawn(thread: Thread) {
    critical_section::with(|cs| {
        THREADS.borrow_ref_mut(cs).insert(thread.id, thread);
    });
}

fn run_interleaving(scheduler: Rc<dyn Fn(usize, &BTreeMap<usize, Thread>) -> usize>) {
    critical_section::with(|cs| {
        *SCHEDULER.borrow_ref_mut(cs) = Some(Scheduler(scheduler));
    });
    trigger_interleaving();
}

#[inline(never)]
fn trigger_interleaving() {
    core::hint::black_box(0);
}

#[inline(never)]
fn thread_finished() {
    core::hint::black_box(0);
}

#[no_mangle]
#[inline(never)]
pub extern "C" fn thread_b() {
    static mut NOT_SHARED_STATE: usize = 0;
    unsafe { NOT_SHARED_STATE = 1 };
}

#[no_mangle]
#[inline(never)]
pub extern "C" fn thread_a() {
    unsafe { SHARED_STATE *= 2 };
    core::hint::black_box(unsafe { SHARED_STATE });
    unsafe { SHARED_STATE += 1 };
}

extern "C" fn umain() -> ! {
    let a_thread = Thread::from_fn(thread_a);
    let b_thread = Thread::from_fn(thread_b);
    spawn(a_thread);
    spawn(b_thread);

    static A_STOP: Mutex<RefCell<usize>> = Mutex::new(RefCell::new(0));

    let scheduler = Rc::new(|current, threads: &BTreeMap<usize, Thread>| {
        critical_section::with(|cs| {
            // If the last thing has been run at least once, try to run the first.
            if current == 0 {
                let borrow_ref = *A_STOP.borrow_ref(cs);
                if threads[&0].current_count >= borrow_ref && threads.contains_key(&1) {
                    // println!("switched, {:?}, {borrow_ref}", threads[&0]);
                    return 1 as usize;
                }
                return 0;
            }
            current
        })
    });
    syscall_enable_breakpoint_mismatch(0);
    run_interleaving(scheduler.clone());
    dbg!(unsafe { SHARED_STATE });

    unsafe { SHARED_STATE = 0 };

    let (a_thread, b_thread) = critical_section::with(|cs| {
        let mut finished = FINISHED_THREADS.borrow_ref_mut(cs);
        dbg!(finished);
        (finished.pop().unwrap(), finished.pop().unwrap())
    });
    assert!(a_thread.finished);
    assert!(b_thread.finished);
    let a_end = a_thread.current_count;
    spawn(a_thread.reset(thread_a));
    spawn(b_thread.reset(thread_b));

    // search over n*m many things
    for a_stop in 0..a_end {
        // for b_stop in 0..b_end {
        syscall_enable_breakpoint_mismatch(0);
        run_interleaving(scheduler.clone());
        dbg!(unsafe { SHARED_STATE });

        let (a_thread, b_thread) = critical_section::with(|cs| {
            let mut finished = FINISHED_THREADS.borrow_ref_mut(cs);
            *A_STOP.borrow_ref_mut(cs) = a_stop;
            unsafe { SHARED_STATE = 0 };
            (finished.pop().unwrap(), finished.pop().unwrap())
        });
        dbg!(a_thread.crc.clone().finalize());
        dbg!(b_thread.crc.clone().finalize());
        assert!(a_thread.finished);
        assert!(b_thread.finished);
        spawn(a_thread.reset(thread_a));
        spawn(b_thread.reset(thread_b));
    }

    disable_single_stepping();
    println!("UMAIN DONE!!!");
    rpi_reboot()
}

pub fn demo() {
    set_watchpoint_status(WatchpointStatus::Disabled);
    set_breakpoint_address(0);
    set_breakpoint_status(BreakpointStatus::Enabled { matching: false });

    run_user_code(umain);
}
