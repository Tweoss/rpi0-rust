//! # Debug hardware functionality.
//!
use bitfield::bitfield;

use crate::{cp_asm_get, cp_asm_set, prefetch_flush, steal_println};

pub fn data_abort_vector(pc: u32) {
    if let WatchpointStatus::Enabled { .. } = get_watchpoint_status() {
        steal_println!("pc = {:04x}", pc);
        set_watchpoint_status(WatchpointStatus::Disabled);
        return;
    }

    panic!("unexpected data abort: pc={}\n", pc);
}

pub fn prefetch_abort_vector(pc: u32) {
    steal_println!("step pc: {:04x}", pc);
    // Disable single stepping.
    if pc == disable_single_stepping as *const () as u32 {
        set_breakpoint_status(BreakpointStatus::Disabled);
    }
    if let BreakpointStatus::Enabled { matching: false } = get_breakpoint_status() {
        // Update the pc for single stepping.
        set_breakpoint_address(pc);
        return;
    }
    panic!("unexpected data abort: pc={}\n", pc);
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
