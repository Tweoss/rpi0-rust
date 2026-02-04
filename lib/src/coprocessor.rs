use bitfield::bitfield;

use crate::{cp_asm_get, cp_asm_set, cp_asm_set_raw, prefetch_flush};

bitfield! {
    /// CP 15 register.
    /// arm1176.pdf, 3-44
    pub struct CP15CR(u32);
    /// Whether or not subpage AP bits are disabled.
    pub get_xp_disabled, set_xp_disabled: 23;
    // should we flush icache?
    pub get_instruction_cache_enabled, set_instruction_cache_enabled: 12;
    pub get_branch_prediction_enabled, set_branch_prediction_enabled: 11;
    pub get_mmu_enabled, set_mmu_enabled: 0;
}

pub type CPSR = CP15CR;

impl CP15CR {
    pub fn read() -> Self {
        Self(sys_control_control_register_get())
    }

    pub fn write(v: Self) {
        sys_control_control_register_set(v.0);
    }
}

cp_asm_get!(sys_control_control_register_get, p15, 0, c1, c0, 0);
cp_asm_set!(sys_control_control_register_set, p15, 0, c1, c0, 0);

cp_asm_set_raw!(pub set_vector_table_base, p15, 0, c12, c0, 0);

bitfield! {
    /// Debug status and control register.
    pub struct DSCR(u32);
    pub get_monitor_debug_mode_enabled, set_monitor_debug_mode_enabled: 15;
    pub get_halting_debug_mode_selected, set_halting_debug_mode_selected: 14;
    pub get_interrupts_disabled, set_interrupts_disabled: 11;
}

impl DSCR {
    pub fn read() -> Self {
        Self(dscr_get())
    }
    pub fn write(v: Self) {
        dscr_set(v.0)
    }
}

cp_asm_get!(dscr_get, p14, 0, c0, c1, 0);
cp_asm_set!(dscr_set, p14, 0, c0, c1, 0);

bitfield! {
    /// Domain Access Control Register
    /// arm1176, 3-63
    pub struct DACR(u32);
    pub get_d15, set_d15: 31,30;
    pub get_d14, set_d14: 29,28;
    pub get_d13, set_d13: 27,26;
    pub get_d12, set_d12: 25,24;
    pub get_d11, set_d11: 23,22;
    pub get_d10, set_d10: 21,20;
    pub get_d9, set_d9: 19,18;
    pub get_d8, set_d8: 17,16;
    pub get_d7, set_d7: 15,14;
    pub get_d6, set_d6: 13,12;
    pub get_d5, set_d5: 11,10;
    pub get_d4, set_d4: 9,8;
    pub get_d3, set_d3: 7,6;
    pub get_d2, set_d2: 5,4;
    pub get_d1, set_d1: 3,2;
    pub get_d0, set_d0: 1,0;
}

/// See arm1176, 3-63
pub enum Domain {
    /// Any access generates a domain fault.
    NoAccess = 0b00,
    /// Accesses are checked against TLB entry access permissions.
    Client = 0b01,
    /// Accesses are not checked, no domain faults.
    Manager = 0b11,
}

impl DACR {
    pub fn read() -> Self {
        Self(dacr_get())
    }
    pub fn write(v: Self) {
        dacr_set(v.0)
    }
}

cp_asm_get!(dacr_get, p15, 0, c3, c0, 0);
cp_asm_set!(dacr_set, p15, 0, c3, c0, 0);

mod lockdown {
    use crate::{cp_asm_get, cp_asm_set_raw};
    use bitfield::bitfield;

    cp_asm_set_raw!(pub lockdown_index_set, p15, 5, c15, c4, 2);

    bitfield! {
        /// Lockdown virtual address register 1176 3-149
        #[derive(Clone, Copy)]
        pub struct LockdownVA(u32);
        impl Debug;
        pub get_va_shifted, set_va_shifted: 31,12;
        // pub get_va, set_va: 31,12;
        pub get_global, set_global: 9;
        pub get_asid, set_asid: 7, 0;
    }

    impl LockdownVA {
        pub fn read() -> Self {
            Self(lockdown_va_get())
        }
        pub fn write(v: Self) {
            lockdown_va_set(v.0)
        }
        pub fn get_va(&self) -> u32 {
            self.get_va_shifted() << 12
        }
    }

    cp_asm_get!(lockdown_va_get, p15, 5, c15, c5, 2);
    cp_asm_set_raw!(lockdown_va_set, p15, 5, c15, c5, 2);

    bitfield! {
        /// Lockdown physical address register 1176 3-150
        #[derive(Clone, Copy)]
        pub struct LockdownPA(u32);
        impl Debug;
        pub get_pa_shifted, set_pa_shifted: 31,12;
        pub get_nsa, set_nsa: 9;
        pub get_nstid, set_nstid: 8;
        pub get_size, set_size: 7,6;
        pub get_apx, set_apx: 3;
        pub get_ap, set_ap: 2,1;
        pub get_valid, set_valid: 0;
    }

    impl LockdownPA {
        pub fn read() -> Self {
            Self(lockdown_pa_get())
        }
        pub fn write(v: Self) {
            lockdown_pa_set(v.0)
        }
        pub fn get_pa(&self) -> u32 {
            self.get_pa_shifted() << 12
        }
    }

    cp_asm_get!(lockdown_pa_get, p15, 5, c15, c6, 2);
    cp_asm_set_raw!(lockdown_pa_set, p15, 5, c15, c6, 2);

    bitfield! {
        /// attributes register 1176 3-151
        #[derive(Clone, Copy)]
        pub struct LockdownAttributes(u32);
        pub get_spv, set_spv: 25;
        pub get_domain, set_domain: 10, 7;
        pub get_xn, set_xn: 6;
        pub get_texcbs, set_texcbs: 5, 0;
    }

    impl LockdownAttributes {
        pub fn read() -> Self {
            Self(lockdown_attributes_get())
        }
        pub fn write(v: Self) {
            lockdown_attributes_set(v.0)
        }
    }

    cp_asm_get!(lockdown_attributes_get, p15, 5, c15, c7, 2);
    cp_asm_set_raw!(lockdown_attributes_set, p15, 5, c15, c7, 2);
}
pub use lockdown::*;

cp_asm_get!(pub dfsr_get, p15, 0, c5, c0, 0);
cp_asm_get!(pub far_get, p15, 0, c6, c0, 0);
cp_asm_get!(pub ifar_get, p15, 0, c6, c0, 2);

cp_asm_set!(pub wvr0_set, p14, 0, c0, c0, 6);

bitfield! {
    /// Watchpoint control register.
    pub struct WCR0(u32);
    pub get_linking, set_linking: 20;
    get_watchpoint_matches, set_watchpoint_matches: 15,14;
    pub get_byte_address_select, set_byte_address_select: 8,5;
    pub get_load_stores, set_load_stores: 4,3;
    pub get_access_condition, set_access_condition: 2,1;
    get_enabled, set_enabled: 0;
}

impl WCR0 {
    pub fn write(v: Self) {
        wcr0_set(v.0)
    }
}

cp_asm_get!(wcr0_get, p14, 0, c0, c0, 7);
cp_asm_set!(wcr0_set, p14, 0, c0, c0, 7);

#[derive(Debug)]
pub enum WatchpointStatus {
    Enabled { load: bool, store: bool },
    Disabled,
}

pub fn get_watchpoint_status() -> WatchpointStatus {
    let wcr = WCR0(wcr0_get());
    match (wcr.get_enabled(), wcr.get_load_stores()) {
        (true, v) => WatchpointStatus::Enabled {
            load: v >= 2,
            store: (v & 1) == 1,
        },
        (false, _) => WatchpointStatus::Disabled,
    }
}

pub fn set_watchpoint_status(status: WatchpointStatus) {
    let mut wcr = WCR0(wcr0_get());
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

cp_asm_set!(pub bvr0_set, p14, 0, c0, c0, 4);

bitfield! {
    /// Breakpoint control register.
    pub struct BCR0(u32);
    // Note context id and mismatching cannot both be true.
    pub get_mismatching, set_mismatching: 22;
    pub get_context_id, set_context_id: 21;
    pub get_linking, set_linking: 20;
    pub get_breakpoint_matches, set_breakpoint_matches: 15,14;
    pub get_byte_address_select, set_byte_address_select: 8,5;
    pub get_access_condition, set_access_condition: 2,1;
    get_enabled, set_enabled: 0;
}

impl BCR0 {
    pub fn read() -> Self {
        Self(bcr0_get())
    }
    pub fn write(v: Self) {
        bcr0_set(v.0)
    }
}

cp_asm_get!(bcr0_get, p14, 0, c0, c0, 5);
cp_asm_set!(bcr0_set, p14, 0, c0, c0, 5);

#[derive(Debug)]
pub enum BreakpointStatus {
    Disabled,
    Enabled { matching: bool },
}

pub fn get_breakpoint_status() -> BreakpointStatus {
    let bcr = BCR0(bcr0_get());
    match (bcr.get_enabled(), bcr.get_mismatching()) {
        (true, v) => BreakpointStatus::Enabled { matching: !v },
        (false, _) => BreakpointStatus::Disabled,
    }
}

pub fn set_breakpoint_status(status: BreakpointStatus) {
    let mut bcr = BCR0(bcr0_get());
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
