use core::{
    alloc::{self, GlobalAlloc},
    cell::{LazyCell, RefCell},
};

const HEAP_START: usize = 1024 * 1024;
const ALLOCATED_AMOUNT: usize = 2 * 1024 * 1024;

#[global_allocator]
static mut ALLOCATOR: WrappedBumpAllocator =
    WrappedBumpAllocator(LazyCell::new(|| RefCell::new(BumpAllocator { used: 0 })));

struct WrappedBumpAllocator(LazyCell<RefCell<BumpAllocator>>);
struct BumpAllocator {
    used: usize,
}

// Just take a segment of memory and pretend we own it.
unsafe impl GlobalAlloc for WrappedBumpAllocator {
    unsafe fn alloc(&self, layout: alloc::Layout) -> *mut u8 {
        let mut a = LazyCell::<RefCell<BumpAllocator>>::force(&self.0).borrow_mut();
        let new_used = a.used + layout.size();
        if new_used >= ALLOCATED_AMOUNT {
            return core::ptr::null_mut();
        }
        let new_allocation = (HEAP_START as *mut u8).offset(a.used as isize);
        a.used = new_used;
        new_allocation
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: alloc::Layout) {}
}

#[no_mangle]
/// For some reoason this is required when using alloc.
pub extern "C" fn __aeabi_unwind_cpp_pr0() {
    panic!("unwinding")
}
