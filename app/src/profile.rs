use core::slice;

use crate::writeln;

extern "C" {
    static mut __code_start__: u8;
    static mut __code_end__: u8;
    static mut __data_start__: u8;
    static mut __data_end__: u8;
    static mut __bss_start__: u8;
    static mut __bss_end__: u8;
}

const HEAP_START: usize = 1024 * 1024;
const ALLOCATED_AMOUNT: usize = 2 * 1024 * 1024;

pub struct Gprof {
    buffer: &'static mut [u32],
    pc_start: usize,
}

impl Gprof {
    pub unsafe fn gprof_init() -> Self {
        let start = (&raw const __code_start__);
        let end = (&raw const __code_end__);
        writeln!("using start {:?}, end {:?}", start, end);
        // Just take a segment of memory and pretend we own it.
        Gprof {
            buffer: unsafe {
                slice::from_raw_parts_mut(
                    HEAP_START as *mut u32,
                    end.byte_offset_from(start) as usize,
                )
            },
            pc_start: (&raw const __code_start__) as usize,
        }
    }

    pub fn gprof_inc(&mut self, pc: usize) {
        assert!(pc >= self.pc_start);
        self.buffer[pc - self.pc_start] += 1;
    }

    pub fn gprof_dump(&self) {
        // writeln!()
        // self.buffer[pc - self.pc_start] += 1;
    }

    //     // increment histogram associated w/ pc.
    // //    few lines of code
    // static void gprof_inc(unsigned pc) {
    //     assert(pc >= pc_min && pc <= pc_max);
    //     unimplemented();
    // }

    // // print out all samples whose count > min_val
}
