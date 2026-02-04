use core::{alloc::Layout, arch::global_asm};

use mmu::print_tlb_lockdown_entries;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use segments::to_megabyte;

use crate::{
    coprocessor::{lockdown_index_set, Domain, LockdownAttributes, LockdownPA, LockdownVA, DACR},
    cp_asm_get, cp_asm_set_raw, dbg, println,
    setup::{__code_end__, _start},
};

global_asm!(
    r#"
@ TODO: confirm the dacr_set works and remove this commented block
@ @ set the 16 2-bit access control fields and do any needed coherence.
@ @ (uint32_t d);
@ .globl domain_access_ctrl_set_asm
@ domain_access_ctrl_set_asm:
@     @ mov r0 to domain access control register
@     mcr p15, 0, r0, c3, c0, 0
@     @ r0 should be zero to be used in prefetch flush
@     mov r0, 0
@     @ prefetch flush
@     mcr p15, 0, r0, c7, c5, 4
@     bx lr

@ mmu should be off
.globl mmu_reset
mmu_reset:
    @ Invalidate all caches (ITLB, DTLB, data cache, instruction cache). Do not
    @ clean the data cache since it will potentially write garbage back to
    @ memory)

    @ invalidate data cache
    mov r0, 0
    mcr p15, 0, r0, c7, c6, 0

    @ Taken from 140e: invalidate ICACHE
    @ Work-around for bug in ARMv6 if we have seperate I/D.  Taken from:
    @   https://elixir.bootlin.com/linux/latest/source/arch/arm/mm/cache-v6.S
    @ MUST HAVE INTERRUPTS DISABLED!
    @ XXX: patch feedback implies we need this for other operations too?
    @
    mcr p15, 0, r0, c7, c5, 0   @ /* invalidate entire I-cache */   \
    mcr p15, 0, r0, c7, c5, 0;  @ /* invalidate entire I-cache */   \
    mcr p15, 0, r0, c7, c5, 0;  @ /* invalidate entire I-cache */   \
    mcr p15, 0, r0, c7, c5, 0;  @ /* invalidate entire I-cache */   \
    .rept   11                  @ /* ARM Ltd recommends at least 11 nops */\
    nop                         @                                   \
    .endr

    @ Prefetch flush apparently
    mcr p15, 0, r0, c7, c5, 4
    
    @ @ invalidate instruction TLB
    @ Q: not necessary since doing both I/D TLB below?
    @ mcr 15, 0, Rd, c8, c5, 0 
    @ @ invalidate data TLB
    @ mcr 15, 0, Rd, c8, c6, 0 

    @ invalidate unified TLB or both I/D TLB
    mcr p15, 0, r0, c8, c7, 0

    @ DSB
    mcr p15, 0, r0, c7, c10, 4

    @ joe says no need for btb
    @ arjun says safe to add
    @ Flush Entire Branch Target Cache: 3-79
    mcr p15, 0, r0, c7, c5, 6
    @ DSB
    mcr p15, 0, r0, c7, c10, 4
    @ Flush Prefetch Buffer: 3-79
    mcr p15, 0, r0, c7, c5, 4

    bx lr

.globl mmu_enable_set_asm
mmu_enable_set_asm:
    @ 1176 6-9
    @ read from cp reg 1
    mrc p15, 0, r2, cr1, cr0, 0
    @ disable instruction cache by clearing bit 12
    bic r2, r2, 0x1000
    @ store back to cp reg 1
    mcr p15, 0, r2, cr1, cr0, 0
    @ prefetch flush
    mov r1, 0, 0
    mcr p15, 0, r1, cr7, cr5, 4
    @ invalidate icache: see mmu_reset above
    mcr p15, 0, r1, c7, c5, 0
    mcr p15, 0, r1, c7, c5, 0
    mcr p15, 0, r1, c7, c5, 0
    mcr p15, 0, r1, c7, c5, 0
    .rept   11
    nop
    .endr
    @ dsb for ensuring icache operation completes
    mcr p15, 0, r1, cr7, cr10, 4
    @ take argument from r0 and move to cp reg 1
    mcr p15, 0, r0, cr1, cr0, 0
    @ prefetch flush (cp reg 1 modification)
    mcr p15, 0, r1, cr7, cr5, 4
    @ flush branch target cache (BTB?)
    @ b2-22 (taken from self-modifying)
    mcr p15, 0, r1, cr7, cr5, 6
    @ dsb for ensuring cache operation completes
    mcr p15, 0, r1, cr7, cr10, 4
    @ prefetch flush (cache operation)
    mcr p15, 0, r1, cr7, cr5, 4
    bx lr


.globl mmu_disable_set_asm
mmu_disable_set_asm:
    @ 1176 6-9 and readme
    @ must disable data cache
    @ must clean data cache (writeback)
    @ apparently invalidate icache
    @ read from cp reg 1
    @ prefetch flush

    mov r1, 0, 0
    @ clean and invalidate entire data cache
    mcr p15, 0, r1, cr7, cr14, 0
    @ dsb for ensuring cache operation completes
    mcr p15, 0, r1, cr7, cr10, 4
    @ disable data cache by clearing bit 2
    bic r0, r0, 0b10
    @ take argument from r0 and move to cp reg 1
    @ this should disable mmu
    mcr p15, 0, r0, cr1, cr0, 0
    @ prefetch flush (cp reg 1 modification)
    mcr p15, 0, r1, cr7, cr5, 4
    @ invalidate icache: see mmu_reset above
    mov r1, 0
    mcr p15, 0, r1, c7, c5, 0
    mcr p15, 0, r1, c7, c5, 0
    mcr p15, 0, r1, c7, c5, 0
    mcr p15, 0, r1, c7, c5, 0
    .rept   11
    nop
    .endr
    @ dsb for ensuring dcache, icache operation completes
    mcr p15, 0, r1, cr7, cr10, 4
    @ flush branch target cache (BTB?)
    mcr p15, 0, r1, cr7, cr5, 6
    @ dsb for ensuring cache operation completes
    mcr p15, 0, r1, cr7, cr10, 4
    @ prefetch flush (cache operation)
    mcr p15, 0, r1, cr7, cr5, 4
    bx lr

.globl mmu_sync_pte_mods
mmu_sync_pte_mods:
    @ sequence from armv6 B2-23
    @ clean data cache
    mov r0, 0
    mcr p15, 0, r0, c7, c14, 0
    @ invalidate icache
    mcr p15, 0, r0, c7, c5, 0
    mcr p15, 0, r0, c7, c5, 0
    mcr p15, 0, r0, c7, c5, 0
    mcr p15, 0, r0, c7, c5, 0
    .rept   11
    nop
    .endr
    @ dsb (cache operation)
    mcr p15, 0, r0, c7, c10, 4
    @ invalidate TLB (by modified virtual address)
    mcr p15, 0, r0, c8, c7, 0
    @ flush branch target buffer (1176 3-32)
    mcr p15, 0, r0, c7, c5, 6
    @ dsb (invalidate btb)
    mcr p15, 0, r0, c7, c10, 4
    @ prefetch flush
    mcr p15, 0, r0, c7, c5, 4
    bx lr

@ void cp15_set_procid_ttbr0(uint32_t proc_and_asid, void *pt);

.globl cp15_set_procid_ttbr0
cp15_set_procid_ttbr0:
    @ from readme:
    @  You will write the address of the page table to
    @ the page table register `ttbr0`, set both `TTBR1` and `TTBRD` to `0`.
    @ Note the alignment restriction!

    @ from armv6 b2-25
    @ change asid to 0
    @ prefetch flush
    @ change TTBR0
    @ set ASID to desired value

    @ from arm1176 3-129
    @ MUST DSB before changing asid and IMB after
    mov r2, 0
    @ DSB
    mcr p15, 0, r2, c7, c10, 4
    @ write 0 for context id and asid
    mcr p15, 0, r2, c13, c0, 1
    @ IMB (prefetch flush)
    mcr p15, 0, r2, c7, c5, 4

    @ set TTBR0 to the page table address in r1
    mcr p15, 0, r1, c2, c0, 0
    @ set TTBR1 and TTBRD to 0
    mcr p15, 0, r2, c2, c0, 1
    mcr p15, 0, r2, c2, c0, 2

    @ per armv6 b2-24, need to invalidate BTB after changing TTBR
    @ then need a prefetch flush (BTB operation)
    mcr p15, 0, r2, c7, c5, 6
    mcr p15, 0, r2, c7, c5, 4

    @ set proc id and asid to desired value
    @ TODO: check if should DSB?
    @ DSB
    mcr p15, 0, r2, c7, c10, 4
    @ write proc id and asid
    mcr p15, 0, r0, c13, c0, 1
    @ per armv6 b2-24, need to invalidate BTB after changing proc id
    mcr p15, 0, r2, c7, c5, 6
    @ DSB (why? dunno)
    mcr p15, 0, r2, c7, c10, 4
    @ IMB (prefetch flush)
    mcr p15, 0, r2, c7, c5, 4

    bx lr
"#
);

// #define MB(x) ((x)*1024*1024)

// // These are the default segments (segment = one MB)
// // that need to be mapped for our binaries so far
// // this quarter.
// //
// // these will hold for all our tests today.
// //
// // if we just map these segments we will get faults
// // for stray pointer read/writes outside of this region.

pub mod segments {
    use crate::{interrupts::INT_STACK_ADDR, setup::STACK_ADDR};

    pub const fn to_megabyte(v: usize) -> usize {
        v * 1024 * 1024
    }

    // Code starts at 0x8000 => map first MB.
    pub const CODE: usize = to_megabyte(0);
    pub const HEAP: usize = to_megabyte(1);
    pub const STACK: usize = (STACK_ADDR as usize) - to_megabyte(1);
    /// Interrupt stack.
    pub const INT_STACK: usize = (INT_STACK_ADDR as usize) - to_megabyte(1);

    // the base of the BCM device memory (for GPIO
    // UART, etc).  Three contiguous MB cover it.
    pub const SEG_BCM_0: usize = 0x20000000;
    pub const SEG_BCM_1: usize = SEG_BCM_0 + to_megabyte(1);
    pub const SEG_BCM_2: usize = SEG_BCM_0 + to_megabyte(2);

    // we guarantee this (2MB) is an
    // unmapped address
    // TODO: check
    pub const SEG_ILLEGAL: usize = to_megabyte(3);
}

pub mod mmu {
    use crate::{
        coprocessor::{LockdownAttributes, LockdownPA, LockdownVA, CPSR},
        println,
        virtual_memory::TexCBS,
    };

    use super::lockdown_index_set;

    extern "C" {
        fn mmu_reset();
        fn mmu_enable_set_asm(cpsr: u32);
        fn mmu_disable_set_asm(cpsr: u32);
        fn cp15_set_procid_ttbr0(proc_and_asid: u32, page_table: u32);
    }

    pub fn is_enabled() -> bool {
        // cp15_control_get()
        // You can enable and disable the MMU by writing the M bit, bit 0, of the CP15 Control Register
        // c1. On reset, this bit is cleared to 0, disabling the MMU. This bit, in addition to most of the
        // MMU control parameters, is duplicated as Secure and Non-secure, to ensure a clear and distinct
        // memory management policy in each world.
        // 6.4.1 Enabling the MMU
        // To enable the MMU in one world you must:
        // 1. 2. 3. 4. Program all relevant CP15 registers of the corresponding world.
        // Program first-level and second-level descriptor page tables as required.
        // Disable and invalidate the Instruction Cache for the corresponding world. You can then
        // re-enable the Instruction Cache when you enable the MMU.
        // Enable the MMU by setting bit 0 in the CP15 Control Register in the corresponding world.
        CPSR::read().get_mmu_enabled()
    }

    pub fn init() {
        unsafe { mmu_reset() };
        let mut cpsr = CPSR::read();
        cpsr.set_xp_disabled(true);
        CPSR::write(cpsr);

        let cpsr = CPSR::read();
        assert!(cpsr.get_xp_disabled());
        assert!(!cpsr.get_mmu_enabled());
    }

    pub fn enable() {
        let mut cpsr = CPSR::read();
        assert!(!cpsr.get_mmu_enabled());
        cpsr.set_mmu_enabled(true);
        unsafe {
            mmu_enable_set_asm(cpsr.0);
        }
    }

    pub fn disable() {
        let mut cpsr = CPSR::read();
        assert!(cpsr.get_mmu_enabled());
        cpsr.set_mmu_enabled(false);
        unsafe {
            mmu_disable_set_asm(cpsr.0);
        }
    }

    pub fn get_tlb_lockdown_entries(
    ) -> impl Iterator<Item = (u32, LockdownPA, LockdownVA, LockdownAttributes)> {
        (0..8).map(|i| {
            lockdown_index_set(i);
            (
                i,
                LockdownPA::read(),
                LockdownVA::read(),
                LockdownAttributes::read(),
            )
        })

        // TRACE:lockdown_print_entries:  pinned TLB lockdown entries:
        // TRACE:lockdown_print_entry:   idx=0
        // TRACE:lockdown_print_entry:     va_ent=0x20000200: va=0x20000|G=1|ASID=0
        // TRACE:lockdown_print_entry:     pa_ent=0x200000c3: pa=0x20000|nsa=0|nstid=0|size=11|apx=1|v=1
        // TRACE:lockdown_print_entry:     attr=0x80: dom=1|xn=0|tex=0|C=0|B=0
    }

    pub fn print_tlb_lockdown_entries() {
        println!("TLB Lockdown Entries");
        for (i, pa, va, attr) in get_tlb_lockdown_entries() {
            println!("idx = {i}");
            println!(
                "\tva={:#0x},\tva={:#07x}|G={}|ASID={}",
                va.0,
                va.get_va_shifted(),
                va.get_global(),
                va.get_asid()
            );
            println!(
                "\tpa={:#0x},\tpa={:#07x}|nsa={}|nstid={}|size={}|apx={}|v={}",
                pa.0,
                pa.get_pa_shifted(),
                pa.get_nsa(),
                pa.get_nstid(),
                pa.get_size(),
                pa.get_apx(),
                pa.get_valid(),
            );
            println!(
                "\tattr={:#0x},\tdom={}|xn={}|tex_cbs={:?}",
                attr.0,
                attr.get_domain(),
                attr.get_xn(),
                TryInto::<TexCBS>::try_into(attr.get_texcbs() as u8),
            );
        }
    }

    // setup pid, asid and pt in hardware.
    // must call:
    //  1. before turning MMU on at all
    //  2. when switching address spaces (or asid won't
    //     be correct).
    pub fn set_ctx(pid: u32, asid: u32, page_table: *const u32) {
        assert!((1..64).contains(&asid));
        unsafe {
            cp15_set_procid_ttbr0((pid << 8) | asid, page_table as u32);
        }
    }
}

// armv6 has 16 different domains with their own privileges.
// just pick one for the kernel.
const KERNEL_DOMAIN: u8 = 1;

pub fn allow_segment_illegal_access() {
    let device_memory = PinAttributes {
        global: true,
        asid: 0,
        domain_id: KERNEL_DOMAIN,
        page_size: PageSize::Mb1,
        // 140e starter code uses "MEM_device" but the actual values
        // in the enum are 0b000_0_0 which in the ARM manual 1176 6-15
        // is called strongly ordered
        tex_c_b_s: TexCBS::StronglyOrdered,
        memory_permissions: ApxAP::NoAccessUser,
    };
    pin_mmu_sec(
        7,
        (0xa77acc0 >> 20) << 20,
        (0xa77acc0 >> 20) << 20,
        device_memory,
    );
}
pub fn disallow_segment_illegal_access() {
    lockdown_index_set(7);
    let mut pa_reg = LockdownPA::read();
    pa_reg.set_valid(false);
    LockdownPA::write(pa_reg);
}

/// Interrupts should not be on.
pub fn setup() {
    dbg!(&raw const _start, &raw const __code_end__);
    assert!(
        &raw const __code_end__
            < unsafe { (segments::CODE as *const u8).byte_offset(to_megabyte(1) as isize) }
    );
    assert!(&raw const _start >= segments::CODE as *const u8);
    // TODO: staff creates a zeroed, aligned page
    //
    let block =
        unsafe { alloc::alloc::alloc_zeroed(Layout::from_size_align(to_megabyte(1), 1).unwrap()) };
    assert!(block as usize == to_megabyte(1));
    // allocate a page table with invalid
    // entries.
    //
    // we will TLB pin all valid mappings.
    // if the code is correct, the hardware
    // will never look anything up in the page
    // table.
    //
    // however, if the code is buggy and does a
    // a wild memory access that isn't in any
    // pinnned entry, the hardware would then try
    // to look the address up in the page table
    // pointed to by the tlbwr0 register.
    //
    // if this page table isn't explicitly
    // initialized to invalid entries, the hardware
    // would interpret the garbage bits there
    // valid and potentially insert them (very
    // hard bug to find).
    //
    // to prevent this we allocate a zero-filled
    // page table.
    //  - 4GB/1MB section * 4 bytes = 4096*4.
    //  - zero-initialized will set each entry's
    //    valid bit to 0 (invalid).
    //  - lower 14 bits (16k aligned) as required
    //    by the hardware.
    assert!(block.align_offset(1 << 14) == 0);

    // attribute for device memory (see <pin.h>).  this
    // is needed when pinning device memory:
    //   - permissions: kernel domain, no user access,
    //   - memory rules: strongly ordered, not shared.

    let device_memory = PinAttributes {
        global: true,
        asid: 0,
        domain_id: KERNEL_DOMAIN,
        page_size: PageSize::Mb1,
        // 140e starter code uses "MEM_device" but the actual values
        // in the enum are 0b000_0_0 which in the ARM manual 1176 6-15
        // is called strongly ordered
        tex_c_b_s: TexCBS::StronglyOrdered,
        memory_permissions: ApxAP::NoAccessUser,
    };
    // attribute for kernel memory (see <pin.h>)
    //   - protection: same as device; we don't want the
    //     user to read/write it.
    //   - memory rules: uncached access.  you can start
    //     messing with this for speed, though have to
    //     do cache coherency maintance
    let kernel_memory = PinAttributes {
        global: true,
        asid: 0,
        domain_id: KERNEL_DOMAIN,
        page_size: PageSize::Mb1,
        tex_c_b_s: TexCBS::Uncached,
        memory_permissions: ApxAP::NoAccessUser,
    };

    mmu::init();

    // identity map all segments in one of the available 0..7 TLB pinned entries.
    pin_mmu_sec(0, segments::CODE, segments::CODE, kernel_memory);
    pin_mmu_sec(1, segments::HEAP, segments::HEAP, kernel_memory);
    pin_mmu_sec(2, segments::STACK, segments::STACK, kernel_memory);
    pin_mmu_sec(3, segments::INT_STACK, segments::INT_STACK, kernel_memory);
    // identity map all device memory
    pin_mmu_sec(4, segments::SEG_BCM_0, segments::SEG_BCM_0, device_memory);
    pin_mmu_sec(5, segments::SEG_BCM_1, segments::SEG_BCM_1, device_memory);
    pin_mmu_sec(6, segments::SEG_BCM_2, segments::SEG_BCM_2, device_memory);

    // // ******************************************************
    // // 3. setup virtual address context.
    // //  - domain permissions.
    // //  - page table, asid, pid.

    // // b4-42: give permissions for all domains.
    // // Q4: if you set this to ~0, what happens w.r.t. Q1?
    // // Q5: if you set this to 0, what happens?
    // staff_domain_access_ctrl_set(DOM_client << dom_kern*2);

    let mut dacr = DACR::read();
    // Set everything to 0 (no access) except the domain for the kernel.
    // Check accesses in that domain against TLB.
    dacr.0 = (Domain::Client as u32) << (KERNEL_DOMAIN * 2);
    DACR::write(dacr);
    // set address space id, page table, and pid.
    // note:
    //  - <pid> is ignored by the hw: it's just to help the os.
    //  - <asid> doesn't matter for this test b/c all entries
    //    are global.
    //  - recall the page table has all entries invalid and is
    //    just to catch memory errors.
    // enum { ASID = 1, PID = 128 };
    // TODO: handle page table

    mmu::set_ctx(128, 1, block as *const u32);

    const BX_LR: u32 = 0xe12fff1e;
    unsafe { (0xa77acc0_u32 as *mut u32).write_volatile(BX_LR) };
    mmu::print_tlb_lockdown_entries();

    // 4. turn MMU on/off, checking that it worked.
    println!("about to enable\n");
    for i in 0..6 {
        assert!(!mmu::is_enabled());
        println!("not enabled");
        mmu::enable();
        // const MOV_PC_R0: u32 = 0xe1a0f000_u32;
        extern "C" fn jump() {
            let a = 0xa77acc0_u32;
            unsafe {
                core::arch::asm!(
                    "
                    blx {}
                    ",
                    in(reg) a,
                    out("lr") _,
                )
            }
        }

        extern "C" fn boop() {}
        match i % 3 {
            0 => unsafe { (0xa77acc0_u32 as *mut u32).write_volatile(*(boop as *const u32)) },
            1 => unsafe {
                assert_eq!(
                    (0xa77acc0_u32 as *mut u32).read_volatile(),
                    *(boop as *const u32)
                );
            },
            2 => jump(),
            _ => unreachable!(),
        }

        disallow_segment_illegal_access();

        // this uses: stack, code, data, BCM.
        if mmu::is_enabled() {
            println!("MMU ON: hello from virtual memory!  cnt={i}");
        } else {
            panic!("mmu is not on");
        }
        mmu::disable();
        assert!(!mmu::is_enabled());
        println!("MMU is off!\n");
    }

    //     static void data_abort_handler(regs_t *r) {
    //     uint32_t fault_addr;

    // #if 0
    //     // b4-44
    //     // alternatively you can use the inline assembly raw.
    //     // can be harder to debug.
    //     asm volatile("MRC p15, 0, %0, c6, c0, 0" : "=r" (fault_addr));
    // #else
    //     fault_addr = cp15_fault_addr_get();
    // #endif

    //     // make sure we faulted on the address that should be accessed.
    //     if(fault_addr != illegal_addr)
    //         panic("illegal fault!  expected %x, got %x\n",
    //             illegal_addr, fault_addr);
    //     else
    //         trace("SUCCESS!: got a fault on address=%x\n", fault_addr);

    //     // done with test.
    //     trace("all done: going to reboot\n");
    //     clean_reboot();
    // }
}

// Check if there is a virtual to physical address tranlation entry
// that holds for privileged reads in the current world.
// See arm1176 3-82
cp_asm_set_raw!(va_translation_set, p15, 0, c7, c8, 0);
cp_asm_get!(pa_translation_get, p15, 0, c7, c4, 0);

// // fill this in based on the <1-test-basic-tutorial.c>
// // NOTE:
// //    you'll need to allocate an invalid page table
// void pin_mmu_init(uint32_t domain_reg) {
//     staff_pin_mmu_init(domain_reg);
//     return;
// }
//
// do a manual translation in tlb:
//   1. store result in <result>
//   2. return 1 if entry exists, 0 otherwise.
//
// NOTE: mmu must be on (confusing).
fn tlb_contains_va(va: u32) -> Result<u32, u32> {
    assert!(mmu::is_enabled());

    // 3-79
    assert_eq!(va & 0b111, 0);
    va_translation_set(va);
    let result = pa_translation_get();
    // First bit indicates error.
    if (result & 0b1) == 1 {
        return Err(result);
    }
    Ok(result)
    // // The bottom 10 bits are other stuff.
    // return Some(result & !(0b11_1111_1111));
}
//
// // check that <va> is pinned.
// int pin_exists(uint32_t va, int verbose_p) {
//     if(!mmu_is_enabled())
//         panic("XXX: i think we can only check existence w/ mmu enabled\n");

//     uint32_t r;
//     if(tlb_contains_va(&r, va)) {
//         assert(va == r);
//         return 1;
//     } else {
//         if(verbose_p)
//             output("TLB should have %x: returned %x [reason=%b]\n",
//                 va, r, bits_get(r,1,6));
//         return 0;
//     }
// }

// // look in test <1-test-basic.c> to see what to do.
// // need to set the <asid> before turning VM on and
// // to switch processes.
// void pin_set_context(uint32_t asid) {
//     // put these back
//     // demand(asid > 0 && asid < 64, invalid asid);
//     // demand(null_pt, must setup null_pt --- look at tests);

//     staff_pin_set_context(asid);
// }

// void pin_clear(unsigned idx)  {
//     staff_pin_clear(idx);
// }

/// See arm1176 6-15
#[derive(Clone, Copy, TryFromPrimitive, Debug)]
#[repr(u8)]
enum TexCBS {
    StronglyOrdered = 0b000_0_0,
    Uncached = 0b001_0_0,
    WriteBackNoAlloc = 0b000_1_1,
    WriteThroughNoAlloc = 0b000_1_0,
}

/// See arm1176 3-151
/// access permissions extension and access permissions.
#[derive(Clone, Copy)]
enum ApxAP {
    /// All accesses generate permission fault.
    NoAccess = 0b000,
    /// User accesses generate fault.
    /// Supervisor can R/W.
    NoAccessUser = 0b001,
    /// User writes generate fault.
    /// Supervisor can R/W.
    ReadOnlyUser = 0b010,
    /// Full access, user and supervisor can R/W.
    ReadWriteUser = 0b011,
    /// User accesses generate fault.
    /// Supervisor read only.
    ReadOnlyPrivate = 0b101,
    /// User and supervisor read only.
    ReadOnly = 0b110,
}

#[derive(Clone, Copy)]
enum PageSize {
    Mb16 = 0b00,
    Kb4 = 0b01,
    Kb64 = 0b10,
    Mb1 = 0b11,
}

/// attributes: these get inserted into the TLB.
#[derive(Clone, Copy)]
struct PinAttributes {
    // Global or not
    global: bool,
    asid: u8,
    domain_id: u8,
    page_size: PageSize,
    tex_c_b_s: TexCBS,
    memory_permissions: ApxAP,
}

fn pin_mmu_sec(idx: u32, va: usize, pa: usize, attributes: PinAttributes) {
    let (va, pa) = (va as u32, pa as u32);
    assert!(idx < 8, "lockdown index too large");
    // lower 20 bits should be 0.
    let first_20_bits = (1 << 20) - 1;
    assert!((va & first_20_bits) == 0, "only handling 1MB sections");
    assert!((pa & first_20_bits) == 0, "only handling 1MB sections");
    assert!(attributes.domain_id < 16, "domain must be less than 16");

    // Set lockdown entry by index.
    lockdown_index_set(idx);
    if attributes.global {
        assert_eq!(attributes.asid, 0);
    }
    // Construct lockdown VA register layout (arm1176 3-149
    // let va = va | (attributes.asid as u32) | ((attributes.global as u32) << 9);
    let mut va_reg = LockdownVA(va);
    va_reg.set_asid(attributes.asid as u32);
    va_reg.set_global(attributes.global);
    // lockdown_va_set(va_reg);
    println!("{:?}", va_reg);
    LockdownVA::write(va_reg);

    // Construct lockdown attribute register layout (arm1176 3-151)
    // Note that we leave subpages disabled and execute never false.
    // let att = ((attributes.domain_id as u32) << 7) | (attributes.tex_c_b_s as u32);
    let mut att_reg = LockdownAttributes(0);
    att_reg.set_spv(false);
    att_reg.set_domain(attributes.domain_id as u32);
    att_reg.set_texcbs(attributes.tex_c_b_s as u32);
    LockdownAttributes::write(att_reg);
    // lockdown_attributes_set(att);
    // Construct lockdown PA register layout (arm1176 3-150).
    // Lowest bit is whether or not entry is valid.
    // Default 0 NSA => memory accesses are secure.
    // Default 0 NSTID => page table entry is secure.
    let mut pa_reg = LockdownPA(pa);
    pa_reg.set_size(attributes.page_size as u32);
    pa_reg.set_ap(attributes.memory_permissions as u32);
    pa_reg.set_valid(true);

    LockdownPA::write(pa_reg);

    // let pa = pa
    //     | ((attributes.page_size as u32) << 6)
    //     | ((attributes.memory_permissions as u32) << 1)
    //     | 1;
    // lockdown_pa_set(pa);

    // TODO read back registers
}

// // map <va>-><pa> at TLB index <idx> with attributes <e>
// void pin_mmu_sec(unsigned idx,
//                 uint32_t va,
//                 uint32_t pa,
//                 pin_t e) {

//     demand(idx < 8, lockdown index too large);
//     // lower 20 bits should be 0.
//     demand(bits_get(va, 0, 19) == 0, only handling 1MB sections);
//     demand(bits_get(pa, 0, 19) == 0, only handling 1MB sections);

//     debug("about to map %x->%x\n", va,pa);

//     // delete this and do add your code below.
//     staff_pin_mmu_sec(idx, va, pa, e);
//     return;

//     // these will hold the values you assign for the tlb entries.
//     uint32_t x, va_ent, pa_ent, attr;
//     todo("assign these variables!\n");

//     // put your code here.
//     unimplemented();

// #if 0
//     if((x = lockdown_va_get()) != va_ent)
//         panic("lockdown va: expected %x, have %x\n", va_ent,x);
//     if((x = lockdown_pa_get()) != pa_ent)
//         panic("lockdown pa: expected %x, have %x\n", pa_ent,x);
//     if((x = lockdown_attr_get()) != attr)
//         panic("lockdown attr: expected %x, have %x\n", attr,x);
// #endif
// }
//
//
//
//
// void lockdown_print_entry(unsigned idx) {
//     trace("   idx=%d\n", idx);
//     lockdown_index_set(idx);
//     uint32_t va_ent = lockdown_va_get();
//     uint32_t pa_ent = lockdown_pa_get();
//     unsigned v = bit_get(pa_ent, 0);

//     if(!v) {
//         trace("     [invalid entry %d]\n", idx);
//         return;
//     }

//     // 3-149
//     ...fill in the needed vars...
//     trace("     va_ent=%x: va=%x|G=%d|ASID=%d\n",
//         va_ent, va, G, asid);

//     // 3-150
//     ...fill in the needed vars...
//     trace("     pa_ent=%x: pa=%x|nsa=%d|nstid=%d|size=%b|apx=%b|v=%d\n",
//                 pa_ent, pa, nsa,nstid,size, apx,v);

//     // 3-151
//     ...fill in the needed vars...
//     trace("     attr=%x: dom=%d|xn=%d|tex=%b|C=%d|B=%d\n",
//             attr, dom,xn,tex,C,B);
// }

// void lockdown_print_entries(const char *msg) {
//     trace("-----  <%s> ----- \n", msg);
//     trace("  pinned TLB lockdown entries:\n");
//     for(int i = 0; i < 8; i++)
//         lockdown_print_entry(i);
//     trace("----- ---------------------------------- \n");
// }
