use crate::dsb;

// no dev barrier:
fn timer_get_usec_raw() -> u32 {
    unsafe { (0x20003004 as *mut u32).read_volatile() }
}

// in usec.  the lower 32-bits of the usec
// counter: if you investigate in the broadcom
// doc can see how to get the high 32-bits too.
fn timer_get_usec() -> u32 {
    dsb();
    let u = timer_get_usec_raw();
    dsb();
    u
}

pub fn delay_us(us: u32) {
    let s = timer_get_usec();
    loop {
        let e = timer_get_usec();
        if (e - s) >= us {
            break;
        }
    }
}

// delay in milliseconds
pub fn delay_ms(ms: u32) {
    delay_us(ms * 1000);
}
