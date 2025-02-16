use crate::{cp_asm_get, cp_asm_set_raw};

cp_asm_set_raw!(cycle_counter_init, p15, 0, c15, c12, 0);
cp_asm_get!(cycle_counter_get, p15, 0, c15, c12, 1);

pub fn init() {
    cycle_counter_init(1);
}

pub fn read() -> u32 {
    cycle_counter_get()
}

#[inline]
pub fn delay_until(cycle_number: u32) {
    let a = read();
    let cycles = cycle_number.wrapping_sub(a);
    // Protect from when we pass the cycle before we measure necessary delay.
    assert!(cycles < u32::MAX / 3, "{}", cycles);
    delay(cycles);
}
#[inline]
pub fn delay(cycle_delay: u32) {
    let start = read();
    // Account for cycle counter wrapping.
    while read().wrapping_sub(start) < cycle_delay {}
}
