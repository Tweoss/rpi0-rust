use core::arch::asm;

pub fn enable() {
    let mut r: u32;
    unsafe { asm!("mrc p15, 0, {}, c1, c0, 0", out (reg) r) };
    r |= 1 << 12; // l1 instruction cache
    r |= 1 << 11; // branch prediction
    unsafe { asm!("mcr p15, 0, {}, c1, c0, 0",  in(reg) r) };
}

// should we flush icache?
pub fn disable() {
    let mut r: u32;
    unsafe { asm!("mrc p15, 0, {}, c1, c0, 0", out (reg) r) };
    //r |= 0x1800;
    r &= !(1 << 12); // l1 instruction cache
    r &= !(1 << 11); // branch prediction
    unsafe { asm!("mcr p15, 0, {}, c1, c0, 0", in (reg) r ) };
}

// int caches_is_enabled(void) {
//     unsigned r;
//     asm volatile ("MRC p15, 0, %0, c1, c0, 0" : "=r" (r));

//     return bit_get(r, 12) && bit_get(r,11);
// }
