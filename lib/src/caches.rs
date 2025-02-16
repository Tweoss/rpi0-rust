//! Instruction cache and branch prediction.

use crate::{cp_asm_get, cp_asm_set};

cp_asm_get!(sys_control_control_register_get, p15, 0, c1, c0, 0);
cp_asm_set!(sys_control_control_register_set, p15, 0, c1, c0, 0);

pub fn enable() {
    let mut r = sys_control_control_register_get();
    r |= 1 << 12; // l1 instruction cache
    r |= 1 << 11; // branch prediction
    sys_control_control_register_set(r);
}

// should we flush icache?
pub fn disable() {
    let mut r = sys_control_control_register_get();
    r &= !(1 << 12); // l1 instruction cache
    r &= !(1 << 11); // branch prediction
    sys_control_control_register_set(r);
}
