use core::arch::asm;

pub fn init() {
    let x = 1;
    unsafe { asm!("mcr p15,0,{},c15,c12,0", in (reg) x) };
}

pub fn read() -> u32 {
    let x;
    unsafe { asm!("mrc p15,0,{},c15,c12,1", out (reg) x) };
    x
}

#[inline]
pub fn delay_until(cycle_number: u32) {
    let cycles = cycle_number.wrapping_sub(read());
    delay(cycles);
}
#[inline]
pub fn delay(cycle_delay: u32) {
    let start = read();
    // Account for cycle counter wrapping.
    while read().wrapping_sub(start) < cycle_delay {}
}
