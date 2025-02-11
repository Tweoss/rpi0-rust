use crate::interrupts::{disable_interrupts, enable_interrupts};
use critical_section::RawRestoreState;

struct MyCriticalSection;
critical_section::set_impl!(MyCriticalSection);

unsafe impl critical_section::Impl for MyCriticalSection {
    unsafe fn acquire() -> RawRestoreState {
        disable_interrupts()
    }

    unsafe fn release(token: RawRestoreState) {
        if token {
            // NOTE: for profiling, clearing timer interrupt right before
            // ending the critical section scope could be
            // useful to avoid biasing towards instructions in enable.
            enable_interrupts();
        }
    }
}
