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

/// Get the identifier for this processor
fn get_cpu_string() -> String {
    let mut buf = [0u8; 48];

    unsafe {
        asm!(r#"

            mov eax, 0x80000002
            cpuid
            mov [$0 + 0*4], eax
            mov [$0 + 1*4], ebx
            mov [$0 + 2*4], ecx
            mov [$0 + 3*4], edx

            mov eax, 0x80000003
            cpuid
            mov [$0 + 4*4], eax
            mov [$0 + 5*4], ebx
            mov [$0 + 6*4], ecx
            mov [$0 + 7*4], edx

            mov eax, 0x80000004
            cpuid
            mov [$0 +  8*4], eax
            mov [$0 +  9*4], ebx
            mov [$0 + 10*4], ecx
            mov [$0 + 11*4], edx

        "# :: "r"(buf.as_mut_ptr() as usize) :
        "cc", "memory", "rax", "rbx", "rcx", "rdx" : "volatile", "intel");
    }

    String::from_utf8_lossy(&buf).into()
}

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
    WHV_REGISTER_NAME_WHvX64RegisterXCr0,
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

    pub xcr0: WHV_REGISTER_VALUE,
}

impl WhvpContext {
    /// Gets the linear address for RIP
    pub fn rip(&self) -> u64 {
        unsafe { self.cs.Segment.Base.wrapping_add(self.rip.Reg64) }
    }

    /// Gets the CR3 for the VM with the reserved and PCID bits masked off
    pub fn cr3(&self) -> u64 {
        unsafe { self.cr3.Reg64 & 0xFFFFFFFFFF000 }
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

    /// Supported features for WHVP
    _whvp_features: WHV_CAPABILITY_FEATURES,

    /// Supported processor features
    _proc_features: WHV_PROCESSOR_FEATURES,

    /// Supported xsave features
    /// If xsave is not supported by WHVP this is a `None` value
    xsave_features: Option<WHV_PROCESSOR_XSAVE_FEATURES>,

    /// List of mapped memory regions in the hypervisor
    /// Tuple is (paddr, size), both size and paddr should be 4 KiB aligned
    memory_regions: Vec<(usize, usize)>,

    /// Buffer to hold the dirty bitmaps. We just cache this allocation for
    /// performance
    dirty_bitmap_tmp: Vec<u64>,

    /// Tracks if memory can be dirty. This is set when we run the guest, and
    /// cleared when we clear dirty memory. It's an optimization for not doing
    /// anything to clear dirty memory when we haven't run the hypervisor.
    memory_dirty: bool,
}

impl Whvp {
    /// Create a new WHVP instance with one processor
    pub fn new() -> Self {
        // Print the CPU model string
        print!("Processor model string: {}\n", get_cpu_string());

        // Check if WHVP is present
        let mut present_check: BOOL = 0;
        let mut bread = 0u32;
        let res = unsafe { WHvGetCapability(
            WHV_CAPABILITY_CODE_WHvCapabilityCodeHypervisorPresent,
            &mut present_check as *mut BOOL as *mut c_void,
            std::mem::size_of_val(&present_check) as u32,
            &mut bread) };
        assert!(res == 0, "WHvGetCapability() error: {:#x}", res);
        assert!(bread == std::mem::size_of_val(&present_check) as u32,
            "Failed to get WHvCapabilityCodeHypervisorPresent");
        assert!(present_check != 0,
            "Hypervisor not present, enable it in Windows features");

        // Get WHVP features
        let mut whvp_features: WHV_CAPABILITY_FEATURES =
            unsafe { std::mem::zeroed() };
        let mut bread = 0u32;
        let res = unsafe { WHvGetCapability(
            WHV_CAPABILITY_CODE_WHvCapabilityCodeFeatures,
            &mut whvp_features as *mut WHV_CAPABILITY_FEATURES as *mut c_void,
            std::mem::size_of_val(&whvp_features) as u32,
            &mut bread) };
        assert!(res == 0, "WHvGetCapability() error: {:#x}", res);
        assert!(bread == std::mem::size_of_val(&whvp_features) as u32,
            "Failed to get WHvCapabilityCodeFeatures");

        // Display the feature set for WHVP
        unsafe {
            print!("WHVP detected with features:\n\
                    \tPartialUnmap:       {}\n\
                    \tLocalApicEmulation: {}\n\
                    \tXsave:              {}\n\
                    \tDirtyPageTracking:  {}\n\
                    \tSpeculationControl: {}\n",
                whvp_features.__bindgen_anon_1.PartialUnmap() != 0,
                whvp_features.__bindgen_anon_1.LocalApicEmulation() != 0,
                whvp_features.__bindgen_anon_1.Xsave() != 0,
                whvp_features.__bindgen_anon_1.DirtyPageTracking() != 0,
                whvp_features.__bindgen_anon_1.SpeculationControl() != 0);
        }

        // Get CPU features
        let mut proc_features: WHV_PROCESSOR_FEATURES =
            unsafe { std::mem::zeroed() };
        let mut bread = 0u32;
        let res = unsafe { WHvGetCapability(
            WHV_CAPABILITY_CODE_WHvCapabilityCodeProcessorFeatures,
            &mut proc_features as *mut WHV_PROCESSOR_FEATURES as *mut c_void,
            std::mem::size_of_val(&proc_features) as u32,
            &mut bread) };
        assert!(res == 0, "WHvGetCapability() error: {:#x}", res);
        assert!(bread == std::mem::size_of_val(&proc_features) as u32,
            "Failed to get WHvCapabilityCodeProcessorFeatures");

        unsafe {
            print!("Processor detected with features:\n\
                    \tSse3Support               {}\n\
                    \tLahfSahfSupport           {}\n\
                    \tSsse3Support              {}\n\
                    \tSse4_1Support             {}\n\
                    \tSse4_2Support             {}\n\
                    \tSse4aSupport              {}\n\
                    \tXopSupport                {}\n\
                    \tPopCntSupport             {}\n\
                    \tCmpxchg16bSupport         {}\n\
                    \tAltmovcr8Support          {}\n\
                    \tLzcntSupport              {}\n\
                    \tMisAlignSseSupport        {}\n\
                    \tMmxExtSupport             {}\n\
                    \tAmd3DNowSupport           {}\n\
                    \tExtendedAmd3DNowSupport   {}\n\
                    \tPage1GbSupport            {}\n\
                    \tAesSupport                {}\n\
                    \tPclmulqdqSupport          {}\n\
                    \tPcidSupport               {}\n\
                    \tFma4Support               {}\n\
                    \tF16CSupport               {}\n\
                    \tRdRandSupport             {}\n\
                    \tRdWrFsGsSupport           {}\n\
                    \tSmepSupport               {}\n\
                    \tEnhancedFastStringSupport {}\n\
                    \tBmi1Support               {}\n\
                    \tBmi2Support               {}\n\
                    \tMovbeSupport              {}\n\
                    \tNpiep1Support             {}\n\
                    \tDepX87FPUSaveSupport      {}\n\
                    \tRdSeedSupport             {}\n\
                    \tAdxSupport                {}\n\
                    \tIntelPrefetchSupport      {}\n\
                    \tSmapSupport               {}\n\
                    \tHleSupport                {}\n\
                    \tRtmSupport                {}\n\
                    \tRdtscpSupport             {}\n\
                    \tClflushoptSupport         {}\n\
                    \tClwbSupport               {}\n\
                    \tShaSupport                {}\n\
                    \tX87PointersSavedSupport   {}\n\
                    \tInvpcidSupport            {}\n\
                    \tIbrsSupport               {}\n\
                    \tStibpSupport              {}\n\
                    \tIbpbSupport               {}\n\
                    \tSsbdSupport               {}\n\
                    \tFastShortRepMovSupport    {}\n\
                    \tRdclNo                    {}\n\
                    \tIbrsAllSupport            {}\n\
                    \tSsbNo                     {}\n\
                    \tRsbANo                    {}\n",
                proc_features.__bindgen_anon_1.Sse3Support() != 0,
                proc_features.__bindgen_anon_1.LahfSahfSupport() != 0,
                proc_features.__bindgen_anon_1.Ssse3Support() != 0,
                proc_features.__bindgen_anon_1.Sse4_1Support() != 0,
                proc_features.__bindgen_anon_1.Sse4_2Support() != 0,
                proc_features.__bindgen_anon_1.Sse4aSupport() != 0,
                proc_features.__bindgen_anon_1.XopSupport() != 0,
                proc_features.__bindgen_anon_1.PopCntSupport() != 0,
                proc_features.__bindgen_anon_1.Cmpxchg16bSupport() != 0,
                proc_features.__bindgen_anon_1.Altmovcr8Support() != 0,
                proc_features.__bindgen_anon_1.LzcntSupport() != 0,
                proc_features.__bindgen_anon_1.MisAlignSseSupport() != 0,
                proc_features.__bindgen_anon_1.MmxExtSupport() != 0,
                proc_features.__bindgen_anon_1.Amd3DNowSupport() != 0,
                proc_features.__bindgen_anon_1.ExtendedAmd3DNowSupport() != 0,
                proc_features.__bindgen_anon_1.Page1GbSupport() != 0,
                proc_features.__bindgen_anon_1.AesSupport() != 0,
                proc_features.__bindgen_anon_1.PclmulqdqSupport() != 0,
                proc_features.__bindgen_anon_1.PcidSupport() != 0,
                proc_features.__bindgen_anon_1.Fma4Support() != 0,
                proc_features.__bindgen_anon_1.F16CSupport() != 0,
                proc_features.__bindgen_anon_1.RdRandSupport() != 0,
                proc_features.__bindgen_anon_1.RdWrFsGsSupport() != 0,
                proc_features.__bindgen_anon_1.SmepSupport() != 0,
                proc_features.__bindgen_anon_1.EnhancedFastStringSupport() != 0,
                proc_features.__bindgen_anon_1.Bmi1Support() != 0,
                proc_features.__bindgen_anon_1.Bmi2Support() != 0,
                proc_features.__bindgen_anon_1.MovbeSupport() != 0,
                proc_features.__bindgen_anon_1.Npiep1Support() != 0,
                proc_features.__bindgen_anon_1.DepX87FPUSaveSupport() != 0,
                proc_features.__bindgen_anon_1.RdSeedSupport() != 0,
                proc_features.__bindgen_anon_1.AdxSupport() != 0,
                proc_features.__bindgen_anon_1.IntelPrefetchSupport() != 0,
                proc_features.__bindgen_anon_1.SmapSupport() != 0,
                proc_features.__bindgen_anon_1.HleSupport() != 0,
                proc_features.__bindgen_anon_1.RtmSupport() != 0,
                proc_features.__bindgen_anon_1.RdtscpSupport() != 0,
                proc_features.__bindgen_anon_1.ClflushoptSupport() != 0,
                proc_features.__bindgen_anon_1.ClwbSupport() != 0,
                proc_features.__bindgen_anon_1.ShaSupport() != 0,
                proc_features.__bindgen_anon_1.X87PointersSavedSupport() != 0,
                proc_features.__bindgen_anon_1.InvpcidSupport() != 0,
                proc_features.__bindgen_anon_1.IbrsSupport() != 0,
                proc_features.__bindgen_anon_1.StibpSupport() != 0,
                proc_features.__bindgen_anon_1.IbpbSupport() != 0,
                proc_features.__bindgen_anon_1.SsbdSupport() != 0,
                proc_features.__bindgen_anon_1.FastShortRepMovSupport() != 0,
                proc_features.__bindgen_anon_1.RdclNo() != 0,
                proc_features.__bindgen_anon_1.IbrsAllSupport() != 0,
                proc_features.__bindgen_anon_1.SsbNo() != 0,
                proc_features.__bindgen_anon_1.RsbANo() != 0);
        }

        // Get xsave features
        let mut xsave_features = None;
        if unsafe { whvp_features.__bindgen_anon_1.Xsave() != 0 } {
            let mut tmp: WHV_PROCESSOR_XSAVE_FEATURES =
                unsafe { std::mem::zeroed() };

            let mut bread = 0u32;
            let res = unsafe { WHvGetCapability(
                WHV_CAPABILITY_CODE_WHvCapabilityCodeProcessorXsaveFeatures,
                &mut tmp as *mut WHV_PROCESSOR_XSAVE_FEATURES as *mut c_void,
                std::mem::size_of_val(&tmp) as u32,
                &mut bread) };
            assert!(res == 0, "WHvGetCapability() error: {:#x}", res);
            assert!(bread == std::mem::size_of_val(&tmp) as u32,
                "Failed to get WHvCapabilityCodeProcessorXsaveFeatures");

            unsafe {
                print!("Processor detected with features:\n\
                        \tXsaveSupport              {}\n\
                        \tXsaveoptSupport           {}\n\
                        \tAvxSupport                {}\n\
                        \tAvx2Support               {}\n\
                        \tFmaSupport                {}\n\
                        \tMpxSupport                {}\n\
                        \tAvx512Support             {}\n\
                        \tAvx512DQSupport           {}\n\
                        \tAvx512CDSupport           {}\n\
                        \tAvx512BWSupport           {}\n\
                        \tAvx512VLSupport           {}\n\
                        \tXsaveCompSupport          {}\n\
                        \tXsaveSupervisorSupport    {}\n\
                        \tXcr1Support               {}\n\
                        \tAvx512BitalgSupport       {}\n\
                        \tAvx512IfmaSupport         {}\n\
                        \tAvx512VBmiSupport         {}\n\
                        \tAvx512VBmi2Support        {}\n\
                        \tAvx512VnniSupport         {}\n\
                        \tGfniSupport               {}\n\
                        \tVaesSupport               {}\n\
                        \tAvx512VPopcntdqSupport    {}\n\
                        \tVpclmulqdqSupport         {}\n",
                        tmp.__bindgen_anon_1.XsaveSupport() != 0,
                        tmp.__bindgen_anon_1.XsaveoptSupport() != 0,
                        tmp.__bindgen_anon_1.AvxSupport() != 0,
                        tmp.__bindgen_anon_1.Avx2Support() != 0,
                        tmp.__bindgen_anon_1.FmaSupport() != 0,
                        tmp.__bindgen_anon_1.MpxSupport() != 0,
                        tmp.__bindgen_anon_1.Avx512Support() != 0,
                        tmp.__bindgen_anon_1.Avx512DQSupport() != 0,
                        tmp.__bindgen_anon_1.Avx512CDSupport() != 0,
                        tmp.__bindgen_anon_1.Avx512BWSupport() != 0,
                        tmp.__bindgen_anon_1.Avx512VLSupport() != 0,
                        tmp.__bindgen_anon_1.XsaveCompSupport() != 0,
                        tmp.__bindgen_anon_1.XsaveSupervisorSupport() != 0,
                        tmp.__bindgen_anon_1.Xcr1Support() != 0,
                        tmp.__bindgen_anon_1.Avx512BitalgSupport() != 0,
                        tmp.__bindgen_anon_1.Avx512IfmaSupport() != 0,
                        tmp.__bindgen_anon_1.Avx512VBmiSupport() != 0,
                        tmp.__bindgen_anon_1.Avx512VBmi2Support() != 0,
                        tmp.__bindgen_anon_1.Avx512VnniSupport() != 0,
                        tmp.__bindgen_anon_1.GfniSupport() != 0,
                        tmp.__bindgen_anon_1.VaesSupport() != 0,
                        tmp.__bindgen_anon_1.Avx512VPopcntdqSupport() != 0,
                        tmp.__bindgen_anon_1.VpclmulqdqSupport() != 0);
            }

            xsave_features = Some(tmp);
        }

        // Create a new WHVP partition
        let mut partition: WHV_PARTITION_HANDLE = std::ptr::null_mut();
        let res = unsafe { WHvCreatePartition(&mut partition) };
        if res == 0x80070005u32 as i32 {
            panic!("Windows Hypervisor Platform now requires Admin access, please rerun this application as Administrator!")
        }
        assert!(res == 0, "WHvCreatePartition() error: {:#x}", res);

        // Create the partition object now which will make a destructor if we
        // bail out of subsequent API calls
        let mut ret = Whvp {
            partition,
            virtual_processors: Vec::new(),
            vm_run_overhead: !0,
            _whvp_features: whvp_features,
            _proc_features: proc_features,
            xsave_features,
            memory_regions: Vec::new(),
            dirty_bitmap_tmp: vec![0u64; (4*1024*1024*1024) / (4096 * 64)],
            memory_dirty: false,
        };

        // Register that we only want one processor
        let proc_count = 1u32;
        let res = unsafe { WHvSetPartitionProperty(partition,
            WHV_PARTITION_PROPERTY_CODE_WHvPartitionPropertyCodeProcessorCount,
            &proc_count as *const u32 as *const c_void,
            std::mem::size_of_val(&proc_count) as u32)
        };
        assert!(res == 0, "WHvSetPartitionProperty() error: {:#x}", res);

        // _HV_X64_INTERRUPT_CONTROLLER_STATE
        // APIC emulation
        /*
        let apic_mode = WHV_X64_LOCAL_APIC_EMULATION_MODE_WHvX64LocalApicEmulationModeXApic;
        let res = unsafe { WHvSetPartitionProperty(partition,
            WHV_PARTITION_PROPERTY_CODE_WHvPartitionPropertyCodeLocalApicEmulationMode,
            &apic_mode as *const i32 as *const c_void,
            std::mem::size_of_val(&apic_mode) as u32)
        };
        assert!(res == 0, "WHvSetPartitionProperty() error: {:#x}", res);*/

        // Enable vmexits on certain events
        let mut vmexits: WHV_EXTENDED_VM_EXITS = unsafe { std::mem::zeroed() };
        unsafe {
            vmexits.__bindgen_anon_1.set_ExceptionExit(1);
            vmexits.__bindgen_anon_1.set_X64MsrExit(1);
            vmexits.__bindgen_anon_1.set_X64CpuidExit(1);
        }
        let res = unsafe { WHvSetPartitionProperty(partition,
            WHV_PARTITION_PROPERTY_CODE_WHvPartitionPropertyCodeExtendedVmExits,
            &vmexits as *const WHV_EXTENDED_VM_EXITS as *const c_void,
            std::mem::size_of_val(&vmexits) as u32)
        };
        assert!(res == 0, "WHvSetPartitionProperty() error: {:#x}", res);

        // Set the vmexit bitmap
        let vmexit_bitmap: u64 = 1 << 1;
        let res = unsafe { WHvSetPartitionProperty(partition,
            WHV_PARTITION_PROPERTY_CODE_WHvPartitionPropertyCodeExceptionExitBitmap,
            &vmexit_bitmap as *const u64 as *const c_void,
            std::mem::size_of_val(&vmexit_bitmap) as u32)
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
        // Always mark mapped memory as tracked for dirty changes
        let res = unsafe { WHvMapGpaRange(self.partition,
            backing.as_mut_ptr() as *mut c_void, addr as u64,
            backing.len() as u64, perm | PERM_DIRTY) };
        assert!(res == 0, "WHvMapGpaRange() error: {:#x}", res);

        // Save that we mapped this memory region
        self.memory_regions.push((addr, backing.len()));
    }

    // benchmarking
    // xperf -on base -stackwalk profile
    // <run benchmark>
    // xperf -d trace.etl
    pub fn get_dirty_list(&mut self, dirty_bits_l1: &mut [u64],
            dirty_bits_l2: &mut [u64]) {
        // Nothing to do if we haven't run the hypervisor since the last reset
        if !self.memory_dirty { return; }
        
        unsafe {
            let bitmap = &mut self.dirty_bitmap_tmp;

            assert!(self._whvp_features.__bindgen_anon_1.DirtyPageTracking() != 0,
                "Dirty page tracking not supported in your version of Windows.\
                 You need a modern Windows build (17763 or newer)\
                 Check your version with the `winver` command. To get on this\
                 build as of Jan 2019 you need to be on the SAC (Targeted)\
                 branch. You could also use an insider build (fast or slow)");

            // For each memory region get the dirty bitmap
            for &(paddr, size) in self.memory_regions.iter() {
                // This is paranoid, we should never have non-aligned entries
                // here in the first place.
                assert!(paddr & 0xfff == 0 && size & 0xfff == 0);

                let res = WHvQueryGpaRangeDirtyBitmap(self.partition,
                    paddr as u64, size as u64, bitmap.as_mut_ptr(),
                    std::mem::size_of_val(bitmap.as_slice()) as u32);
                assert!(res == 0,
                    "WHvQueryGpaRangeDirtyBitmap() error: {:#x}", res);

                let qwords_in_map = size / (4096 * 64);
                for (ii, qword) in bitmap[..qwords_in_map]
                        .iter_mut().enumerate() {
                    // Nothing dirty here
                    if *qword == 0 { continue; }

                    // Compute address of this qword
                    let addr = paddr + ii * 4096 * 64;

                    // Set the dirty bit in the first level (1 MiB) dirty bit
                    // table
                    let qword_l1 = addr / (1024 * 1024 * 64);
                    let bit_l1   = (addr / (1024 * 1024)) % 64;
                    dirty_bits_l1[qword_l1] |= 1 << bit_l1;

                    // Set the dirty bits in the 4 KiB list. This has the same
                    // layout as the dirty map we get from
                    // WHvQueryGpaRangeDirtyBitmap so we can just or the bits in
                    let qword_l2 = addr / (4096 * 64);
                    dirty_bits_l2[qword_l2] |= *qword;
                }
            }
        }

        // Memory can no longer be dirty as we've restored it
        self.memory_dirty = false;
    }

    // Run the hypervisor until exit, returning the exit context
    pub fn run(&mut self) -> WHV_RUN_VP_EXIT_CONTEXT {
        let mut context: WHV_RUN_VP_EXIT_CONTEXT =
            unsafe { std::mem::zeroed() };
        let res = unsafe { WHvRunVirtualProcessor(self.partition, 0,
            &mut context as *mut WHV_RUN_VP_EXIT_CONTEXT as *mut c_void,
            std::mem::size_of_val(&context) as u32) };
        assert!(res == 0, "WHvRunVirtualProcessor() error: {:#x}", res);

        // Mark that memory may be dirty
        self.memory_dirty = true;

        context
    }

    // Get the entire WHVP context structure from the hypervisor
    pub fn get_context(&self) -> WhvpContext {
        // Make room for the context
        let mut ret: WhvpContext = unsafe { std::mem::zeroed() };

        // Check if xsave is supported
        let xsave_supported = self.xsave_features.map(|x| {
            unsafe { x.__bindgen_anon_1.XsaveSupport() != 0 }
        });

        // If xsave is not supported make sure we do not sync xcr0
        let names = if xsave_supported == Some(true) {
            WHVP_CONTEXT_NAMES.len()
        } else {
            // Don't use xcr0
            assert!(WHVP_CONTEXT_NAMES[WHVP_CONTEXT_NAMES.len() - 1] == 
                WHV_REGISTER_NAME_WHvX64RegisterXCr0);
            WHVP_CONTEXT_NAMES.len() - 1
        };

        // Get the state
        let res = unsafe { WHvGetVirtualProcessorRegisters(self.partition, 0,
            WHVP_CONTEXT_NAMES.as_ptr(), names as u32,
            &mut ret as *mut WhvpContext as *mut WHV_REGISTER_VALUE) };
        assert!(res == 0, "WHvGetVirtualProcessorRegisters() error: {:#x}", res);
        ret
    }

    // Commit the entire WHVP context structure state to the hypervisor state
    pub fn set_context(&mut self, context: &WhvpContext) {
        // Check if xsave is supported
        let xsave_supported = self.xsave_features.map(|x| {
            unsafe { x.__bindgen_anon_1.XsaveSupport() != 0 }
        });

        // If xsave is not supported make sure we do not sync xcr0
        let names = if xsave_supported == Some(true) {
            WHVP_CONTEXT_NAMES.len()
        } else {
            // Don't use xcr0
            assert!(WHVP_CONTEXT_NAMES[WHVP_CONTEXT_NAMES.len() - 1] == 
                WHV_REGISTER_NAME_WHvX64RegisterXCr0);
            WHVP_CONTEXT_NAMES.len() - 1
        };

        // Apply the state
        let res = unsafe { WHvSetVirtualProcessorRegisters(self.partition, 0,
            WHVP_CONTEXT_NAMES.as_ptr(), names as u32,
            context as *const WhvpContext as *const WHV_REGISTER_VALUE) };

        if res != 0 {
            print!("Likely unsupported virtual processor register\n");
            print!("Please open a ticket with your CPU string:\n");
            print!("    CPU string: \"{}\"\n", get_cpu_string());

            assert!(res == 0,
                "WHvSetVirtualProcessorRegisters() error: {:#x}\n{}",
                res, context);
        }
    }

    /// Request that an exception is delivered to the guest based on `vector`
    /// and `error_code`. If `error_code` is `None` then no error code will
    /// be pushed onto the stack for the exception
    pub fn deliver_exception(&mut self, vector: u8, error_code: Option<u32>) {
        // List of names
        const REGINT_NAMES: &[i32] = &[
            WHV_REGISTER_NAME_WHvRegisterPendingEvent
        ];

        let mut event: WHV_X64_PENDING_EXCEPTION_EVENT =
            unsafe { std::mem::zeroed() };

        // Get current exception event state
        let res = unsafe { WHvGetVirtualProcessorRegisters(self.partition, 0,
            REGINT_NAMES.as_ptr(), REGINT_NAMES.len() as u32,
            &mut event as *mut WHV_X64_PENDING_EXCEPTION_EVENT as
            *mut WHV_REGISTER_VALUE) };
        assert!(res == 0,
            "WHvGetVirtualProcessorRegisters() error: {:#x}",
            res);

        unsafe {
            // Make sure there's not already a pending exception event
            assert!(event.__bindgen_anon_1.EventPending() == 0,
                "Can't deliver exception when one is already pending");

            event.__bindgen_anon_1.set_EventPending(1);
            event.__bindgen_anon_1.set_EventType(
                WHV_X64_PENDING_EVENT_TYPE_WHvX64PendingEventException as u32);
            event.__bindgen_anon_1.set_DeliverErrorCode(
                if error_code.is_none() { 0 } else { 1 });
            event.__bindgen_anon_1.set_Vector(vector as u32);
            event.__bindgen_anon_1.ErrorCode = error_code.unwrap_or(0);
            event.__bindgen_anon_1.ExceptionParameter = 0; // What is this?

            let res = WHvSetVirtualProcessorRegisters(self.partition,
                0, REGINT_NAMES.as_ptr(), REGINT_NAMES.len() as u32,
                &event as *const WHV_X64_PENDING_EXCEPTION_EVENT as
                *const WHV_REGISTER_VALUE);
            assert!(res == 0,
                "WHvSetVirtualProcessorRegisters() error: {:#x}",
                res);
        }
    }

    /// Clear a pending exception
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
