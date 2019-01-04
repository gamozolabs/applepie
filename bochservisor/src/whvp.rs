/// This file is used to hide some of the internal WHVP workings from Rust and
/// provide a more Rust-like interface
/// 
/// There isn't much anything special here, just a lot of FFI

use std::ffi::c_void;
use crate::time;
use whvp_bindings::winhvplatform::*;

// Force a dependency on winhvplatform.lib to make sure we link against it
#[link(name = "winhvplatform")] extern {}

// Memory permissions
pub const PERM_NONE:    i32 = WHV_MAP_GPA_RANGE_FLAGS_WHvMapGpaRangeFlagNone;
pub const PERM_READ:    i32 = WHV_MAP_GPA_RANGE_FLAGS_WHvMapGpaRangeFlagRead;
pub const PERM_WRITE:   i32 = WHV_MAP_GPA_RANGE_FLAGS_WHvMapGpaRangeFlagWrite;
pub const PERM_EXECUTE: i32 = WHV_MAP_GPA_RANGE_FLAGS_WHvMapGpaRangeFlagExecute;
pub const PERM_DIRTY:   i32 = WHV_MAP_GPA_RANGE_FLAGS_WHvMapGpaRangeFlagTrackDirtyPages;

/// Entire context name list for a processor using all possible fields WHVP
/// allows access to
/// 
/// DO NOT CHANGE WITHOUT CHANGING THE C VERSION IN BOCHS!!!
const WHVP_CONTEXT_NAMES: &[i32] = &[
    WHV_REGISTER_NAME_WHvX64RegisterRax,
    WHV_REGISTER_NAME_WHvX64RegisterRcx,
    WHV_REGISTER_NAME_WHvX64RegisterRdx,
    WHV_REGISTER_NAME_WHvX64RegisterRbx,
    WHV_REGISTER_NAME_WHvX64RegisterRsp,
    WHV_REGISTER_NAME_WHvX64RegisterRbp,
    WHV_REGISTER_NAME_WHvX64RegisterRsi,
    WHV_REGISTER_NAME_WHvX64RegisterRdi,
    WHV_REGISTER_NAME_WHvX64RegisterR8,
    WHV_REGISTER_NAME_WHvX64RegisterR9,
    WHV_REGISTER_NAME_WHvX64RegisterR10,
    WHV_REGISTER_NAME_WHvX64RegisterR11,
    WHV_REGISTER_NAME_WHvX64RegisterR12,
    WHV_REGISTER_NAME_WHvX64RegisterR13,
    WHV_REGISTER_NAME_WHvX64RegisterR14,
    WHV_REGISTER_NAME_WHvX64RegisterR15,
    WHV_REGISTER_NAME_WHvX64RegisterRip,
    WHV_REGISTER_NAME_WHvX64RegisterRflags,
    WHV_REGISTER_NAME_WHvX64RegisterEs,
    WHV_REGISTER_NAME_WHvX64RegisterCs,
    WHV_REGISTER_NAME_WHvX64RegisterSs,
    WHV_REGISTER_NAME_WHvX64RegisterDs,
    WHV_REGISTER_NAME_WHvX64RegisterFs,
    WHV_REGISTER_NAME_WHvX64RegisterGs,
    WHV_REGISTER_NAME_WHvX64RegisterLdtr,
    WHV_REGISTER_NAME_WHvX64RegisterTr,
    WHV_REGISTER_NAME_WHvX64RegisterIdtr,
    WHV_REGISTER_NAME_WHvX64RegisterGdtr,
    WHV_REGISTER_NAME_WHvX64RegisterCr0,
    WHV_REGISTER_NAME_WHvX64RegisterCr2,
    WHV_REGISTER_NAME_WHvX64RegisterCr3,
    WHV_REGISTER_NAME_WHvX64RegisterCr4,
    WHV_REGISTER_NAME_WHvX64RegisterCr8,
    WHV_REGISTER_NAME_WHvX64RegisterDr0,
    WHV_REGISTER_NAME_WHvX64RegisterDr1,
    WHV_REGISTER_NAME_WHvX64RegisterDr2,
    WHV_REGISTER_NAME_WHvX64RegisterDr3,
    WHV_REGISTER_NAME_WHvX64RegisterDr6,
    WHV_REGISTER_NAME_WHvX64RegisterDr7,
    WHV_REGISTER_NAME_WHvX64RegisterXCr0,
    WHV_REGISTER_NAME_WHvX64RegisterXmm0,
    WHV_REGISTER_NAME_WHvX64RegisterXmm1,
    WHV_REGISTER_NAME_WHvX64RegisterXmm2,
    WHV_REGISTER_NAME_WHvX64RegisterXmm3,
    WHV_REGISTER_NAME_WHvX64RegisterXmm4,
    WHV_REGISTER_NAME_WHvX64RegisterXmm5,
    WHV_REGISTER_NAME_WHvX64RegisterXmm6,
    WHV_REGISTER_NAME_WHvX64RegisterXmm7,
    WHV_REGISTER_NAME_WHvX64RegisterXmm8,
    WHV_REGISTER_NAME_WHvX64RegisterXmm9,
    WHV_REGISTER_NAME_WHvX64RegisterXmm10,
    WHV_REGISTER_NAME_WHvX64RegisterXmm11,
    WHV_REGISTER_NAME_WHvX64RegisterXmm12,
    WHV_REGISTER_NAME_WHvX64RegisterXmm13,
    WHV_REGISTER_NAME_WHvX64RegisterXmm14,
    WHV_REGISTER_NAME_WHvX64RegisterXmm15,
    WHV_REGISTER_NAME_WHvX64RegisterFpMmx0,
    WHV_REGISTER_NAME_WHvX64RegisterFpMmx1,
    WHV_REGISTER_NAME_WHvX64RegisterFpMmx2,
    WHV_REGISTER_NAME_WHvX64RegisterFpMmx3,
    WHV_REGISTER_NAME_WHvX64RegisterFpMmx4,
    WHV_REGISTER_NAME_WHvX64RegisterFpMmx5,
    WHV_REGISTER_NAME_WHvX64RegisterFpMmx6,
    WHV_REGISTER_NAME_WHvX64RegisterFpMmx7,
    WHV_REGISTER_NAME_WHvX64RegisterFpControlStatus,
    WHV_REGISTER_NAME_WHvX64RegisterXmmControlStatus,
    WHV_REGISTER_NAME_WHvX64RegisterTsc,
    WHV_REGISTER_NAME_WHvX64RegisterEfer,
    WHV_REGISTER_NAME_WHvX64RegisterKernelGsBase,
    WHV_REGISTER_NAME_WHvX64RegisterApicBase,
    WHV_REGISTER_NAME_WHvX64RegisterPat,
    WHV_REGISTER_NAME_WHvX64RegisterSysenterCs,
    WHV_REGISTER_NAME_WHvX64RegisterSysenterEip,
    WHV_REGISTER_NAME_WHvX64RegisterSysenterEsp,
    WHV_REGISTER_NAME_WHvX64RegisterStar,
    WHV_REGISTER_NAME_WHvX64RegisterLstar,
    WHV_REGISTER_NAME_WHvX64RegisterCstar,
    WHV_REGISTER_NAME_WHvX64RegisterSfmask,
    WHV_REGISTER_NAME_WHvX64RegisterTscAux,
    //WHV_REGISTER_NAME_WHvX64RegisterSpecCtrl,
    //WHV_REGISTER_NAME_WHvX64RegisterPredCmd,
    //WHV_REGISTER_NAME_WHvX64RegisterApicId,
    //WHV_REGISTER_NAME_WHvX64RegisterApicVersion,
    //WHV_REGISTER_NAME_WHvRegisterPendingInterruption,
    //WHV_REGISTER_NAME_WHvRegisterInterruptState,
    //WHV_REGISTER_NAME_WHvRegisterPendingEvent,
    //WHV_REGISTER_NAME_WHvX64RegisterDeliverabilityNotifications,
    //WHV_REGISTER_NAME_WHvRegisterInternalActivityState,
];

/// Entire context structure for a processor using all possible fields WHVP
/// allows access to
/// 
/// It seems internally there's some alignment requirements.
/// We force this by using a 64-byte alignment
/// 
/// DO NOT CHANGE WITHOUT CHANGING THE C VERSION IN BOCHS!!!
/// This structure crosses FFI boundaries!
#[repr(C, align(64))]
pub struct WhvpContext {
    pub rax: WHV_REGISTER_VALUE,
    pub rcx: WHV_REGISTER_VALUE,
    pub rdx: WHV_REGISTER_VALUE,
    pub rbx: WHV_REGISTER_VALUE,
    pub rsp: WHV_REGISTER_VALUE,
    pub rbp: WHV_REGISTER_VALUE,
    pub rsi: WHV_REGISTER_VALUE,
    pub rdi: WHV_REGISTER_VALUE,
    pub r8:  WHV_REGISTER_VALUE,
    pub r9:  WHV_REGISTER_VALUE,
    pub r10: WHV_REGISTER_VALUE,
    pub r11: WHV_REGISTER_VALUE,
    pub r12: WHV_REGISTER_VALUE,
    pub r13: WHV_REGISTER_VALUE,
    pub r14: WHV_REGISTER_VALUE,
    pub r15: WHV_REGISTER_VALUE,
    pub rip: WHV_REGISTER_VALUE,

    pub rflags: WHV_REGISTER_VALUE,

    pub es: WHV_REGISTER_VALUE,
    pub cs: WHV_REGISTER_VALUE,
    pub ss: WHV_REGISTER_VALUE,
    pub ds: WHV_REGISTER_VALUE,
    pub fs: WHV_REGISTER_VALUE,
    pub gs: WHV_REGISTER_VALUE,

    pub ldtr: WHV_REGISTER_VALUE,
    pub tr:   WHV_REGISTER_VALUE,
    pub idtr: WHV_REGISTER_VALUE,
    pub gdtr: WHV_REGISTER_VALUE,

    pub cr0: WHV_REGISTER_VALUE,
    pub cr2: WHV_REGISTER_VALUE,
    pub cr3: WHV_REGISTER_VALUE,
    pub cr4: WHV_REGISTER_VALUE,
    pub cr8: WHV_REGISTER_VALUE,

    pub dr0: WHV_REGISTER_VALUE,
    pub dr1: WHV_REGISTER_VALUE,
    pub dr2: WHV_REGISTER_VALUE,
    pub dr3: WHV_REGISTER_VALUE,
    pub dr6: WHV_REGISTER_VALUE,
    pub dr7: WHV_REGISTER_VALUE,

    pub xcr0: WHV_REGISTER_VALUE,

    pub xmm0: WHV_REGISTER_VALUE,
    pub xmm1: WHV_REGISTER_VALUE,
    pub xmm2: WHV_REGISTER_VALUE,
    pub xmm3: WHV_REGISTER_VALUE,
    pub xmm4: WHV_REGISTER_VALUE,
    pub xmm5: WHV_REGISTER_VALUE,
    pub xmm6: WHV_REGISTER_VALUE,
    pub xmm7: WHV_REGISTER_VALUE,
    pub xmm8: WHV_REGISTER_VALUE,
    pub xmm9: WHV_REGISTER_VALUE,
    pub xmm10: WHV_REGISTER_VALUE,
    pub xmm11: WHV_REGISTER_VALUE,
    pub xmm12: WHV_REGISTER_VALUE,
    pub xmm13: WHV_REGISTER_VALUE,
    pub xmm14: WHV_REGISTER_VALUE,
    pub xmm15: WHV_REGISTER_VALUE,

    pub st0: WHV_REGISTER_VALUE,
    pub st1: WHV_REGISTER_VALUE,
    pub st2: WHV_REGISTER_VALUE,
    pub st3: WHV_REGISTER_VALUE,
    pub st4: WHV_REGISTER_VALUE,
    pub st5: WHV_REGISTER_VALUE,
    pub st6: WHV_REGISTER_VALUE,
    pub st7: WHV_REGISTER_VALUE,

    pub fp_control:  WHV_REGISTER_VALUE,
    pub xmm_control: WHV_REGISTER_VALUE,

    pub tsc: WHV_REGISTER_VALUE,
    pub efer: WHV_REGISTER_VALUE,
    pub kernel_gs_base: WHV_REGISTER_VALUE,
    pub apic_base: WHV_REGISTER_VALUE,
    pub pat: WHV_REGISTER_VALUE,
    pub sysenter_cs: WHV_REGISTER_VALUE,
    pub sysenter_eip: WHV_REGISTER_VALUE,
    pub sysenter_esp: WHV_REGISTER_VALUE,
    pub star: WHV_REGISTER_VALUE,
    pub lstar: WHV_REGISTER_VALUE,
    pub cstar: WHV_REGISTER_VALUE,
    pub sfmask: WHV_REGISTER_VALUE,

    pub tsc_aux: WHV_REGISTER_VALUE,
    //pub spec_ctrl: WHV_REGISTER_VALUE, not yet supported by Windows 17763
    //pub pred_cmd: WHV_REGISTER_VALUE, not yet supported by Windows 17763
    //pub apic_id: WHV_REGISTER_VALUE, not yet supported by Windows 17763
    //pub apic_version: WHV_REGISTER_VALUE, not yet supported by Windows 17763
    //pub pending_interruption: WHV_REGISTER_VALUE,
    //pub interrupt_state: WHV_REGISTER_VALUE,
    //pub pending_event: WHV_REGISTER_VALUE,
    //pub deliverability_notifications: WHV_REGISTER_VALUE,
    //pub internal_activity_state: WHV_REGISTER_VALUE, unknown type
}

impl WhvpContext {
    /// Gets the linear address for RIP
    pub fn rip(&self) -> u64 {
        unsafe { self.cs.Segment.Base.wrapping_add(self.rip.Reg64) }
    }
}

impl Default for WhvpContext {
    /// Returns a zeroed out context structure
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

impl std::fmt::Display for WhvpContext {
    /// Pretty prints the entire WHVP state available
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        unsafe {
            write!(f,
                "rax {:016x} rcx {:016x} rdx {:016x} rbx {:016x}\n\
                 rsp {:016x} rbp {:016x} rsi {:016x} rdi {:016x}\n\
                 r8  {:016x} r9  {:016x} r10 {:016x} r11 {:016x}\n\
                 r12 {:016x} r13 {:016x} r14 {:016x} r15 {:016x}\n\
                 rip {:04x}:{:016x} (linear {:016x})\n\
                 rfl {:016x}\n\
                 es  {:04x} base {:016x} limit {:08x} attr {:04x}\n\
                 cs  {:04x} base {:016x} limit {:08x} attr {:04x}\n\
                 ss  {:04x} base {:016x} limit {:08x} attr {:04x}\n\
                 ds  {:04x} base {:016x} limit {:08x} attr {:04x}\n\
                 fs  {:04x} base {:016x} limit {:08x} attr {:04x}\n\
                 gs  {:04x} base {:016x} limit {:08x} attr {:04x}\n\
                 ldtr base {:016x} limit {:04x} attr {:04x}\n\
                 tr   base {:016x} limit {:04x} attr {:04x}\n\
                 idtr base {:016x} limit {:04x}\n\
                 gdtr base {:016x} limit {:04x}\n\
                 tsc {:016x} tsc aux {:016x} efer {:016x} kernel gs {:016x}\n\
                 apic base {:016x} pat {:016x}\n\
                 sysenter eip {:04x}:{:08x} esp {:08x}\n\
                 star {:016x} lstar {:016x} cstar {:016x} sfmask {:016x}\n\
                 cr0 {:016x} cr2 {:016x} cr3 {:016x} cr4 {:016x}\n\
                 cr8 {:016x}\n\
                 dr0 {:016x} dr1 {:016x} dr2 {:016x} dr3 {:016x}\n\
                 dr6 {:016x} dr7 {:016x}\n\
                 xcr0 {:016x}\n\
                 xmm control {:08x} mask {:08x}\n\
                 xmm0  {:08x} {:08x} {:08x} {:08x}\n\
                 xmm1  {:08x} {:08x} {:08x} {:08x}\n\
                 xmm2  {:08x} {:08x} {:08x} {:08x}\n\
                 xmm3  {:08x} {:08x} {:08x} {:08x}\n\
                 xmm4  {:08x} {:08x} {:08x} {:08x}\n\
                 xmm5  {:08x} {:08x} {:08x} {:08x}\n\
                 xmm6  {:08x} {:08x} {:08x} {:08x}\n\
                 xmm7  {:08x} {:08x} {:08x} {:08x}\n\
                 xmm8  {:08x} {:08x} {:08x} {:08x}\n\
                 xmm9  {:08x} {:08x} {:08x} {:08x}\n\
                 xmm10 {:08x} {:08x} {:08x} {:08x}\n\
                 xmm11 {:08x} {:08x} {:08x} {:08x}\n\
                 xmm12 {:08x} {:08x} {:08x} {:08x}\n\
                 xmm13 {:08x} {:08x} {:08x} {:08x}\n\
                 xmm14 {:08x} {:08x} {:08x} {:08x}\n\
                 xmm15 {:08x} {:08x} {:08x} {:08x}\n\
                 fp control {:04x} status {:04x} tag {:02x} last op {:04x}\n\
                 fp0 mantissa {:016x} exponent {:04x} sign {:02x}\n\
                 fp1 mantissa {:016x} exponent {:04x} sign {:02x}\n\
                 fp2 mantissa {:016x} exponent {:04x} sign {:02x}\n\
                 fp3 mantissa {:016x} exponent {:04x} sign {:02x}\n\
                 fp4 mantissa {:016x} exponent {:04x} sign {:02x}\n\
                 fp5 mantissa {:016x} exponent {:04x} sign {:02x}\n\
                 fp6 mantissa {:016x} exponent {:04x} sign {:02x}\n\
                 fp7 mantissa {:016x} exponent {:04x} sign {:02x}\n\
                 ",
                 /*
                 interrupt pending {:x} type {:x} deliver error {:x}\n    \
                     inst len {:x} nested {:x} vector {:04x}\n    \
                     error code {:08x}\n\
                 interrupt state shadow {:x} nmi masked {:x}\n\
                 pending event {:08x}\n\
                 deliverability nmi {:x} interrupt {:x} priority {:x}\n\*/
                self.rax.Reg64, self.rcx.Reg64, self.rdx.Reg64,
                self.rbx.Reg64, self.rsp.Reg64, self.rbp.Reg64,
                self.rsi.Reg64, self.rdi.Reg64, self.r8.Reg64,
                self.r9.Reg64,  self.r10.Reg64, self.r11.Reg64,
                self.r12.Reg64, self.r13.Reg64, self.r14.Reg64,
                self.r15.Reg64,
                self.cs.Segment.Selector, self.rip.Reg64,
                self.rip.Reg64.wrapping_add(self.cs.Segment.Base),
                self.rflags.Reg64,
                self.es.Segment.Selector, self.es.Segment.Base,
                self.es.Segment.Limit, self.es.Segment.__bindgen_anon_1.Attributes,
                self.cs.Segment.Selector, self.cs.Segment.Base,
                self.cs.Segment.Limit, self.cs.Segment.__bindgen_anon_1.Attributes,
                self.ss.Segment.Selector, self.ss.Segment.Base,
                self.ss.Segment.Limit, self.ss.Segment.__bindgen_anon_1.Attributes,
                self.ds.Segment.Selector, self.ds.Segment.Base,
                self.ds.Segment.Limit, self.ds.Segment.__bindgen_anon_1.Attributes,
                self.fs.Segment.Selector, self.fs.Segment.Base,
                self.fs.Segment.Limit, self.fs.Segment.__bindgen_anon_1.Attributes,
                self.gs.Segment.Selector, self.gs.Segment.Base,
                self.gs.Segment.Limit, self.gs.Segment.__bindgen_anon_1.Attributes,
                self.ldtr.Segment.Base, self.ldtr.Segment.Limit,
                self.ldtr.Segment.__bindgen_anon_1.Attributes,
                self.tr.Segment.Base, self.tr.Segment.Limit,
                self.tr.Segment.__bindgen_anon_1.Attributes,
                self.idtr.Table.Base, self.idtr.Table.Limit,
                self.gdtr.Table.Base, self.gdtr.Table.Limit,
                self.tsc.Reg64, self.tsc_aux.Reg64, self.efer.Reg64,
                self.kernel_gs_base.Reg64,
                self.apic_base.Reg64, self.pat.Reg64,
                self.sysenter_cs.Reg64, self.sysenter_eip.Reg64,
                self.sysenter_esp.Reg64,
                self.star.Reg64, self.lstar.Reg64, self.cstar.Reg64,
                self.sfmask.Reg64,
                self.cr0.Reg64, self.cr2.Reg64, self.cr3.Reg64,
                self.cr4.Reg64, self.cr8.Reg64, self.dr0.Reg64,
                self.dr1.Reg64, self.dr2.Reg64, self.dr3.Reg64,
                self.dr6.Reg64, self.dr7.Reg64,
                self.xcr0.Reg64,
                self.xmm_control.XmmControlStatus.__bindgen_anon_1.XmmStatusControl,
                self.xmm_control.XmmControlStatus.__bindgen_anon_1.XmmStatusControlMask,
                // Missing LastFpRdp and LastFpDp depending on mode
                self.xmm0.Reg128.Dword[0], self.xmm0.Reg128.Dword[1],
                self.xmm0.Reg128.Dword[2], self.xmm0.Reg128.Dword[3],
                self.xmm1.Reg128.Dword[0], self.xmm1.Reg128.Dword[1],
                self.xmm1.Reg128.Dword[2], self.xmm1.Reg128.Dword[3],
                self.xmm2.Reg128.Dword[0], self.xmm2.Reg128.Dword[1],
                self.xmm2.Reg128.Dword[2], self.xmm2.Reg128.Dword[3],
                self.xmm3.Reg128.Dword[0], self.xmm3.Reg128.Dword[1],
                self.xmm3.Reg128.Dword[2], self.xmm3.Reg128.Dword[3],
                self.xmm4.Reg128.Dword[0], self.xmm4.Reg128.Dword[1],
                self.xmm4.Reg128.Dword[2], self.xmm4.Reg128.Dword[3],
                self.xmm5.Reg128.Dword[0], self.xmm5.Reg128.Dword[1],
                self.xmm5.Reg128.Dword[2], self.xmm5.Reg128.Dword[3],
                self.xmm6.Reg128.Dword[0], self.xmm6.Reg128.Dword[1],
                self.xmm6.Reg128.Dword[2], self.xmm6.Reg128.Dword[3],
                self.xmm7.Reg128.Dword[0], self.xmm7.Reg128.Dword[1],
                self.xmm7.Reg128.Dword[2], self.xmm7.Reg128.Dword[3],
                self.xmm8.Reg128.Dword[0], self.xmm8.Reg128.Dword[1],
                self.xmm8.Reg128.Dword[2], self.xmm8.Reg128.Dword[3],
                self.xmm9.Reg128.Dword[0], self.xmm9.Reg128.Dword[1],
                self.xmm9.Reg128.Dword[2], self.xmm9.Reg128.Dword[3],
                self.xmm10.Reg128.Dword[0], self.xmm10.Reg128.Dword[1],
                self.xmm10.Reg128.Dword[2], self.xmm10.Reg128.Dword[3],
                self.xmm11.Reg128.Dword[0], self.xmm11.Reg128.Dword[1],
                self.xmm11.Reg128.Dword[2], self.xmm11.Reg128.Dword[3],
                self.xmm12.Reg128.Dword[0], self.xmm12.Reg128.Dword[1],
                self.xmm12.Reg128.Dword[2], self.xmm12.Reg128.Dword[3],
                self.xmm13.Reg128.Dword[0], self.xmm13.Reg128.Dword[1],
                self.xmm13.Reg128.Dword[2], self.xmm13.Reg128.Dword[3],
                self.xmm14.Reg128.Dword[0], self.xmm14.Reg128.Dword[1],
                self.xmm14.Reg128.Dword[2], self.xmm14.Reg128.Dword[3],
                self.xmm15.Reg128.Dword[0], self.xmm15.Reg128.Dword[1],
                self.xmm15.Reg128.Dword[2], self.xmm15.Reg128.Dword[3],
                self.fp_control.FpControlStatus.__bindgen_anon_1.FpControl,
                self.fp_control.FpControlStatus.__bindgen_anon_1.FpStatus,
                self.fp_control.FpControlStatus.__bindgen_anon_1.FpTag,
                self.fp_control.FpControlStatus.__bindgen_anon_1.LastFpOp,
                // TODO: Missing LastFpRip/LastFpEip based on processor mode
                self.st0.Fp.__bindgen_anon_1.Mantissa,
                self.st0.Fp.__bindgen_anon_1.BiasedExponent(),
                self.st0.Fp.__bindgen_anon_1.Sign(),
                self.st1.Fp.__bindgen_anon_1.Mantissa,
                self.st1.Fp.__bindgen_anon_1.BiasedExponent(),
                self.st1.Fp.__bindgen_anon_1.Sign(),
                self.st2.Fp.__bindgen_anon_1.Mantissa,
                self.st2.Fp.__bindgen_anon_1.BiasedExponent(),
                self.st2.Fp.__bindgen_anon_1.Sign(),
                self.st3.Fp.__bindgen_anon_1.Mantissa,
                self.st3.Fp.__bindgen_anon_1.BiasedExponent(),
                self.st3.Fp.__bindgen_anon_1.Sign(),
                self.st4.Fp.__bindgen_anon_1.Mantissa,
                self.st4.Fp.__bindgen_anon_1.BiasedExponent(),
                self.st4.Fp.__bindgen_anon_1.Sign(),
                self.st5.Fp.__bindgen_anon_1.Mantissa,
                self.st5.Fp.__bindgen_anon_1.BiasedExponent(),
                self.st5.Fp.__bindgen_anon_1.Sign(),
                self.st6.Fp.__bindgen_anon_1.Mantissa,
                self.st6.Fp.__bindgen_anon_1.BiasedExponent(),
                self.st6.Fp.__bindgen_anon_1.Sign(),
                self.st7.Fp.__bindgen_anon_1.Mantissa,
                self.st7.Fp.__bindgen_anon_1.BiasedExponent(),
                self.st7.Fp.__bindgen_anon_1.Sign(),
            )
        }
    }
}

/// Structure representing an instance of a hypervisor using the WHVP API
pub struct Whvp {
    /// The raw partition used to manage the partition with the WHVP API
    partition: WHV_PARTITION_HANDLE,

    /// List of allocated virtual processors
    virtual_processors: Vec<u32>,

    /// Number of cycles to enter the VM, fault, and exit
    vm_run_overhead: u64,
}

impl Whvp {
    /// Create a new WHVP instance with one processor
    pub fn new() -> Self {
        // Create a new WHVP partition
        let mut partition: WHV_PARTITION_HANDLE = std::ptr::null_mut();
        let res = unsafe { WHvCreatePartition(&mut partition) };
        assert!(res == 0, "WHvCreatePartition() error: {:#x}", res);

        // Create the partition object now which will make a destructor if we
        // bail out of subsequent API calls
        let mut ret = Whvp {
            partition,
            virtual_processors: Vec::new(),
            vm_run_overhead: !0,
        };

        // Register that we only want one processor
        let proc_count = 1u32;
        let res = unsafe { WHvSetPartitionProperty(partition,
            WHV_PARTITION_PROPERTY_CODE_WHvPartitionPropertyCodeProcessorCount,
            &proc_count as *const u32 as *const c_void,
            std::mem::size_of_val(&proc_count) as u32)
        };
        assert!(res == 0, "WHvSetPartitionProperty() error: {:#x}", res);

        // Enable vmexits on certain events
        let mut vmexits: WHV_EXTENDED_VM_EXITS = unsafe { std::mem::zeroed() };
        unsafe {
            vmexits.__bindgen_anon_1.set_ExceptionExit(0);
            vmexits.__bindgen_anon_1.set_X64MsrExit(0);
            vmexits.__bindgen_anon_1.set_X64CpuidExit(0);
        }
        let res = unsafe { WHvSetPartitionProperty(partition,
            WHV_PARTITION_PROPERTY_CODE_WHvPartitionPropertyCodeExtendedVmExits,
            &vmexits as *const WHV_EXTENDED_VM_EXITS as *const c_void,
            std::mem::size_of_val(&vmexits) as u32)
        };
        assert!(res == 0, "WHvSetPartitionProperty() error: {:#x}", res);

        // Setup the partition, not sure what this does but it's just how the
        // API works
        let res = unsafe { WHvSetupPartition(partition) };
        assert!(res == 0, "WHvSetupPartition() error: {:#x}", res);

        // Create a single virtual processor
        let res = unsafe { WHvCreateVirtualProcessor(partition, 0, 0) };
        assert!(res == 0, "WHvCreateVirtualProcessor() error: {:#x}", res);
        ret.virtual_processors.push(0);

        // Time the approximate overhead of running a VM. We use this to get
        // a more accurate estimate of how many cycles actually executed inside
        // the hypervisor rather than just the API and context switches.
        for _ in 0..10000 {
            let start = time::rdtsc();
            ret.run();
            let elapsed = time::rdtsc() - start;
            ret.vm_run_overhead = std::cmp::min(ret.vm_run_overhead, elapsed);
        }

        ret
    }

    /// Get the raw WHVP handle
    pub fn handle(&self) -> WHV_PARTITION_HANDLE {
        self.partition
    }

    /// Gets the number of cycles of overhead for a VM entry.
    /// 
    /// This can be used to more accurately estimate the amount of cycles spent
    /// in the hypervisor when subtracted from a cycle count of time spent in
    /// a `vm.run()`
    pub fn overhead(&self) -> u64 {
        self.vm_run_overhead
    }

    /// Request that the hypervisor exits as soon as it's back in an
    /// interruptable state. This allows us to get the guest into a state where
    /// we can deliver things like timer interrupts.
    pub fn register_interrupt_window(&mut self) {
        // List of names, in this case just the
        // RegisterDeliverabilityNotifications will be changed.
        const REGINT_NAMES: &[i32] = &[
            WHV_REGISTER_NAME_WHvX64RegisterDeliverabilityNotifications
        ];

        unsafe {
            // Set that we want an interrupt notification
            let mut reg_value: WHV_REGISTER_VALUE = std::mem::zeroed();
            reg_value.DeliverabilityNotifications
                .__bindgen_anon_1.set_InterruptNotification(1);

            // Call the API to apply the changes
            let res = WHvSetVirtualProcessorRegisters(self.partition, 0,
                REGINT_NAMES.as_ptr(), REGINT_NAMES.len() as u32,
                &reg_value as *const WHV_REGISTER_VALUE);
            assert!(res == 0,
                "WHvSetVirtualProcessorRegisters() error: {:#x}", res);
        }
    }

    /// Map a new region of memory at physical address `addr` backed by the
    /// memory pointed to by `backing` for the size of `backing.len()` using
    /// `perm`. `perm` is a bitwise or-ed combination `PERM_READ`,
    /// `PERM_WRITE`, and `PERM_EXECUTE`.
    pub fn map_memory(&mut self, addr: usize, backing: &mut [u8], perm: i32) {
        // Make sure everything looks sane about this new mapping
        assert!(addr & 0xfff == 0,
            "Cannot map page-unaligned memory");
        assert!((backing.as_ptr() as usize) & 0xfff == 0,
            "Cannot map page-unaligned memory (unaligned backing)");
        assert!(backing.len() & 0xfff == 0,
            "Cannot map page-unaligned memory (unaligned size)");
        assert!(backing.len() > 0, "Cannot map zero bytes");

        // Map the memory in!
        let res = unsafe { WHvMapGpaRange(self.partition,
            backing.as_mut_ptr() as *mut c_void, addr as u64,
            backing.len() as u64, perm) };
        assert!(res == 0, "WHvMapGpaRange() error: {:#x}", res);
    }

    // Run the hypervisor until exit, returning the exit context
    pub fn run(&mut self) -> WHV_RUN_VP_EXIT_CONTEXT {
        let mut context: WHV_RUN_VP_EXIT_CONTEXT =
            unsafe { std::mem::zeroed() };
        let res = unsafe { WHvRunVirtualProcessor(self.partition, 0,
            &mut context as *mut WHV_RUN_VP_EXIT_CONTEXT as *mut c_void,
            std::mem::size_of_val(&context) as u32) };
        assert!(res == 0, "WHvRunVirtualProcessor() error: {:#x}", res);
        context
    }

    // Get the entire WHVP context structure from the hypervisor
    pub fn get_context(&self) -> WhvpContext {
        // Make room for the context
        let mut ret: WhvpContext = unsafe { std::mem::zeroed() };

        // Get the state
        let res = unsafe { WHvGetVirtualProcessorRegisters(self.partition, 0,
            WHVP_CONTEXT_NAMES.as_ptr(), WHVP_CONTEXT_NAMES.len() as u32,
            &mut ret as *mut WhvpContext as *mut WHV_REGISTER_VALUE) };
        assert!(res == 0, "WHvGetVirtualProcessorRegisters() error: {:#x}", res);
        ret
    }

    // Commit the entire WHVP context structure state to the hypervisor state
    pub fn set_context(&mut self, context: &WhvpContext) {
        // Apply the state
        let res = unsafe { WHvSetVirtualProcessorRegisters(self.partition, 0,
            WHVP_CONTEXT_NAMES.as_ptr(), WHVP_CONTEXT_NAMES.len() as u32,
            context as *const WhvpContext as *const WHV_REGISTER_VALUE) };
        assert!(res == 0,
            "WHvSetVirtualProcessorRegisters() error: {:#x}\n{}",
            res, context);
    }

    // Clear a pending exception
    pub fn clear_pending_exception(&mut self) {
        // List of names
        const REGINT_NAMES: &[i32] = &[
            WHV_REGISTER_NAME_WHvRegisterPendingEvent
        ];

        let event: WHV_X64_PENDING_EXCEPTION_EVENT =
            unsafe { std::mem::zeroed() };

        let res = unsafe { WHvSetVirtualProcessorRegisters(self.partition, 0,
            REGINT_NAMES.as_ptr(), REGINT_NAMES.len() as u32,
            &event as *const WHV_X64_PENDING_EXCEPTION_EVENT as *const WHV_REGISTER_VALUE) };
        assert!(res == 0,
            "WHvSetVirtualProcessorRegisters() error: {:#x}",
            res);
    }
}

impl Drop for Whvp {
    /// Drop everything related to the WHVP API we registered
    fn drop(&mut self) {
        // Delete all virtual processors
        for &pid in &self.virtual_processors {
            let res = unsafe { WHvDeleteVirtualProcessor(self.partition, pid) };
            assert!(res == 0, "WHvDeleteVirtualProcessor() error: {:#x}", res);
        }

        // Delete the partition itself
        let res = unsafe { WHvDeletePartition(self.partition) };
        assert!(res == 0, "WHvDeletePartition() error: {:#x}", res);
    }
}
