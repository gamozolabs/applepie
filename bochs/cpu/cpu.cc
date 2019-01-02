/////////////////////////////////////////////////////////////////////////
// $Id$
/////////////////////////////////////////////////////////////////////////
//
//  Copyright (C) 2001-2018  The Bochs Project
//
//  This library is free software; you can redistribute it and/or
//  modify it under the terms of the GNU Lesser General Public
//  License as published by the Free Software Foundation; either
//  version 2 of the License, or (at your option) any later version.
//
//  This library is distributed in the hope that it will be useful,
//  but WITHOUT ANY WARRANTY; without even the implied warranty of
//  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
//  Lesser General Public License for more details.
//
//  You should have received a copy of the GNU Lesser General Public
//  License along with this library; if not, write to the Free Software
//  Foundation, Inc., 51 Franklin St, Fifth Floor, Boston, MA B 02110-1301 USA
/////////////////////////////////////////////////////////////////////////

#define NEED_CPU_REG_SHORTCUTS 1
#include "bochs.h"
#include "cpu.h"
#define LOG_THIS BX_CPU_THIS_PTR

#include "cpustats.h"
#include "param_names.h"

#include <WinHvPlatform.h>

// Big context stucture. This must stay in sync with the Rust version as this
// is passed between FFI boundaries
__declspec(align(64))
struct _whvp_context {
  WHV_REGISTER_VALUE rax;
  WHV_REGISTER_VALUE rcx;
  WHV_REGISTER_VALUE rdx;
  WHV_REGISTER_VALUE rbx;
  WHV_REGISTER_VALUE rsp;
  WHV_REGISTER_VALUE rbp;
  WHV_REGISTER_VALUE rsi;
  WHV_REGISTER_VALUE rdi;
  WHV_REGISTER_VALUE r8;
  WHV_REGISTER_VALUE r9;
  WHV_REGISTER_VALUE r10;
  WHV_REGISTER_VALUE r11;
  WHV_REGISTER_VALUE r12;
  WHV_REGISTER_VALUE r13;
  WHV_REGISTER_VALUE r14;
  WHV_REGISTER_VALUE r15;
  WHV_REGISTER_VALUE rip;

  WHV_REGISTER_VALUE rflags;

  WHV_REGISTER_VALUE es;
  WHV_REGISTER_VALUE cs;
  WHV_REGISTER_VALUE ss;
  WHV_REGISTER_VALUE ds;
  WHV_REGISTER_VALUE fs;
  WHV_REGISTER_VALUE gs;

  WHV_REGISTER_VALUE ldtr;
  WHV_REGISTER_VALUE tr;
  WHV_REGISTER_VALUE idtr;
  WHV_REGISTER_VALUE gdtr;

  WHV_REGISTER_VALUE cr0;
  WHV_REGISTER_VALUE cr2;
  WHV_REGISTER_VALUE cr3;
  WHV_REGISTER_VALUE cr4;
  WHV_REGISTER_VALUE cr8;

  WHV_REGISTER_VALUE dr0;
  WHV_REGISTER_VALUE dr1;
  WHV_REGISTER_VALUE dr2;
  WHV_REGISTER_VALUE dr3;
  WHV_REGISTER_VALUE dr6;
  WHV_REGISTER_VALUE dr7;

  WHV_REGISTER_VALUE xcr0;

  WHV_REGISTER_VALUE xmm0;
  WHV_REGISTER_VALUE xmm1;
  WHV_REGISTER_VALUE xmm2;
  WHV_REGISTER_VALUE xmm3;
  WHV_REGISTER_VALUE xmm4;
  WHV_REGISTER_VALUE xmm5;
  WHV_REGISTER_VALUE xmm6;
  WHV_REGISTER_VALUE xmm7;
  WHV_REGISTER_VALUE xmm8;
  WHV_REGISTER_VALUE xmm9;
  WHV_REGISTER_VALUE xmm10;
  WHV_REGISTER_VALUE xmm11;
  WHV_REGISTER_VALUE xmm12;
  WHV_REGISTER_VALUE xmm13;
  WHV_REGISTER_VALUE xmm14;
  WHV_REGISTER_VALUE xmm15;

  WHV_REGISTER_VALUE st0;
  WHV_REGISTER_VALUE st1;
  WHV_REGISTER_VALUE st2;
  WHV_REGISTER_VALUE st3;
  WHV_REGISTER_VALUE st4;
  WHV_REGISTER_VALUE st5;
  WHV_REGISTER_VALUE st6;
  WHV_REGISTER_VALUE st7;

  WHV_REGISTER_VALUE fp_control;
  WHV_REGISTER_VALUE xmm_control;

  WHV_REGISTER_VALUE tsc;
  WHV_REGISTER_VALUE efer;
  WHV_REGISTER_VALUE kernel_gs_base;
  WHV_REGISTER_VALUE apic_base;
  WHV_REGISTER_VALUE pat;
  WHV_REGISTER_VALUE sysenter_cs;
  WHV_REGISTER_VALUE sysenter_eip;
  WHV_REGISTER_VALUE sysenter_esp;
  WHV_REGISTER_VALUE star;
  WHV_REGISTER_VALUE lstar;
  WHV_REGISTER_VALUE cstar;
  WHV_REGISTER_VALUE sfmask;

  WHV_REGISTER_VALUE tsc_aux;
  //WHV_REGISTER_VALUE spec_ctrl;
  //WHV_REGISTER_VALUE pred_cmd;
  //WHV_REGISTER_VALUE apic_id;
  //WHV_REGISTER_VALUE apic_version;
  //WHV_REGISTER_VALUE pending_interruption;
  //WHV_REGISTER_VALUE interrupt_state;
  //WHV_REGISTER_VALUE pending_event;
  //WHV_REGISTER_VALUE deliverability_notifications;
  //WHV_REGISTER_VALUE internal_activity_state;
};

// Function pointers passed to the Rust DLL for accessing things they need in
// the Bochs environment
struct _bochs_routines {
  void  (*set_context)(const struct _whvp_context*);
  void  (*get_context)(struct _whvp_context*);
  void  (*step_device)(Bit64u steps);
  void  (*step_cpu)(Bit64u steps);
  void* (*get_memory_backing)(Bit64u address, int type);
};

// Set helper for bochs segments
#define SET_SEGMENT_FULL(name, bochs_seg) \
  BX_CPU_THIS_PTR set_segment_ar_data(&bochs_seg,\
    context->name.Segment.Present,\
    context->name.Segment.Selector,\
    context->name.Segment.Base,\
    context->name.Segment.Limit,\
    context->name.Segment.Attributes);

// Set helper for floating pointer registers
#define SET_FP_REG(name, bochs_idx) \
  BX_CPU_THIS_PTR the_i387.st_space[bochs_idx].fraction  = context->name.Fp.Mantissa;\
  BX_CPU_THIS_PTR the_i387.st_space[bochs_idx].exp       = context->name.Fp.BiasedExponent;\
  BX_CPU_THIS_PTR the_i387.st_space[bochs_idx].exp      |= (context->name.Fp.Sign << 15);

// get_memory_backing implementation for Rust which allows Rust to get access to
// memory backings for certain physical addresses
void* get_memory_backing(Bit64u address, int type) {
  return (void*)BX_CPU_THIS_PTR getHostMemAddr(address, type);
}

// set_context implementation that allows Rust to provide a new CPU context for
// Bochs to use internally
void set_context(const struct _whvp_context* context) {
  RAX = context->rax.Reg64;
  RCX = context->rcx.Reg64;
  RDX = context->rdx.Reg64;
  RBX = context->rbx.Reg64;
  RSP = context->rsp.Reg64;
  RBP = context->rbp.Reg64;
  RSI = context->rsi.Reg64;
  RDI = context->rdi.Reg64;
  R8  = context->r8.Reg64;
  R9  = context->r9.Reg64;
  R10 = context->r10.Reg64;
  R11 = context->r11.Reg64;
  R12 = context->r12.Reg64;
  R13 = context->r13.Reg64;
  R14 = context->r14.Reg64;
  R15 = context->r15.Reg64;
  RIP = context->rip.Reg64;
  BX_CPU_THIS_PTR setEFlags(context->rflags.Reg32);

  SET_SEGMENT_FULL(es, BX_CPU_THIS_PTR sregs[BX_SEG_REG_ES]);
  SET_SEGMENT_FULL(cs, BX_CPU_THIS_PTR sregs[BX_SEG_REG_CS]);
  SET_SEGMENT_FULL(ss, BX_CPU_THIS_PTR sregs[BX_SEG_REG_SS]);
  SET_SEGMENT_FULL(ds, BX_CPU_THIS_PTR sregs[BX_SEG_REG_DS]);
  SET_SEGMENT_FULL(fs, BX_CPU_THIS_PTR sregs[BX_SEG_REG_FS]);
  SET_SEGMENT_FULL(gs, BX_CPU_THIS_PTR sregs[BX_SEG_REG_GS]);
  SET_SEGMENT_FULL(ldtr, BX_CPU_THIS_PTR ldtr);
  SET_SEGMENT_FULL(tr, BX_CPU_THIS_PTR tr);

  BX_CPU_THIS_PTR idtr.base  = context->idtr.Table.Base;
  BX_CPU_THIS_PTR idtr.limit = context->idtr.Table.Limit;
  BX_CPU_THIS_PTR gdtr.base  = context->gdtr.Table.Base;
  BX_CPU_THIS_PTR gdtr.limit = context->gdtr.Table.Limit;

  BX_CPU_THIS_PTR cr0.set32(context->cr0.Reg32);
  BX_CPU_THIS_PTR cr2 = context->cr2.Reg64;
  BX_CPU_THIS_PTR cr3 = context->cr3.Reg64;
  BX_CPU_THIS_PTR cr4.set32(context->cr4.Reg32);
  BX_CPU_THIS_PTR lapic.set_tpr((context->cr8.Reg32 & 0xf) << 4);

  BX_CPU_THIS_PTR dr[0] = context->dr0.Reg64;
  BX_CPU_THIS_PTR dr[1] = context->dr1.Reg64;
  BX_CPU_THIS_PTR dr[2] = context->dr2.Reg64;
  BX_CPU_THIS_PTR dr[3] = context->dr3.Reg64;
  BX_CPU_THIS_PTR dr6.set32(context->dr6.Reg32);
  BX_CPU_THIS_PTR dr7.set32(context->dr7.Reg32);

  BX_CPU_THIS_PTR xcr0.set32(context->xcr0.Reg32);

  memcpy(BX_READ_XMM_REG(0).xmm_u32, context->xmm0.Reg128.Dword, 16);
  memcpy(BX_READ_XMM_REG(1).xmm_u32, context->xmm1.Reg128.Dword, 16);
  memcpy(BX_READ_XMM_REG(2).xmm_u32, context->xmm2.Reg128.Dword, 16);
  memcpy(BX_READ_XMM_REG(3).xmm_u32, context->xmm3.Reg128.Dword, 16);
  memcpy(BX_READ_XMM_REG(4).xmm_u32, context->xmm4.Reg128.Dword, 16);
  memcpy(BX_READ_XMM_REG(5).xmm_u32, context->xmm5.Reg128.Dword, 16);
  memcpy(BX_READ_XMM_REG(6).xmm_u32, context->xmm6.Reg128.Dword, 16);
  memcpy(BX_READ_XMM_REG(7).xmm_u32, context->xmm7.Reg128.Dword, 16);
  memcpy(BX_READ_XMM_REG(8).xmm_u32, context->xmm8.Reg128.Dword, 16);
  memcpy(BX_READ_XMM_REG(9).xmm_u32, context->xmm9.Reg128.Dword, 16);
  memcpy(BX_READ_XMM_REG(10).xmm_u32, context->xmm10.Reg128.Dword, 16);
  memcpy(BX_READ_XMM_REG(11).xmm_u32, context->xmm11.Reg128.Dword, 16);
  memcpy(BX_READ_XMM_REG(12).xmm_u32, context->xmm12.Reg128.Dword, 16);
  memcpy(BX_READ_XMM_REG(13).xmm_u32, context->xmm13.Reg128.Dword, 16);
  memcpy(BX_READ_XMM_REG(14).xmm_u32, context->xmm14.Reg128.Dword, 16);
  memcpy(BX_READ_XMM_REG(15).xmm_u32, context->xmm15.Reg128.Dword, 16);

  SET_FP_REG(st0, 0);
  SET_FP_REG(st1, 1);
  SET_FP_REG(st2, 2);
  SET_FP_REG(st3, 3);
  SET_FP_REG(st4, 4);
  SET_FP_REG(st5, 5);
  SET_FP_REG(st6, 6);
  SET_FP_REG(st7, 7);

  BX_CPU_THIS_PTR the_i387.cwd = context->fp_control.FpControlStatus.FpControl;
  BX_CPU_THIS_PTR the_i387.swd = context->fp_control.FpControlStatus.FpStatus;
  BX_CPU_THIS_PTR the_i387.twd = context->fp_control.FpControlStatus.FpTag;
  BX_CPU_THIS_PTR the_i387.foo = context->fp_control.FpControlStatus.LastFpOp;

  if(BX_CPU_THIS_PTR efer.get_LMA()) {
    // Long mode state
    BX_CPU_THIS_PTR the_i387.fip = context->fp_control.FpControlStatus.LastFpRip;
  } else {
    // Other mode state
    BX_CPU_THIS_PTR the_i387.fip = context->fp_control.FpControlStatus.LastFpEip;
    BX_CPU_THIS_PTR the_i387.fcs = context->fp_control.FpControlStatus.LastFpCs;
  }

  BX_CPU_THIS_PTR mxcsr.mxcsr = context->xmm_control.XmmControlStatus.XmmStatusControl;
  BX_CPU_THIS_PTR mxcsr_mask  = context->xmm_control.XmmControlStatus.XmmStatusControlMask;

  if(BX_CPU_THIS_PTR efer.get_LMA()) {
    // Long mode state
    BX_CPU_THIS_PTR the_i387.fdp = context->xmm_control.XmmControlStatus.LastFpRdp;
  } else {
    // Other mode state
    BX_CPU_THIS_PTR the_i387.fdp = context->xmm_control.XmmControlStatus.LastFpDp;
    BX_CPU_THIS_PTR the_i387.fds = context->xmm_control.XmmControlStatus.LastFpDs;
  }

  BX_CPU_THIS_PTR set_TSC(context->tsc.Reg64);
  BX_CPU_THIS_PTR efer.set32(context->efer.Reg32);
  BX_CPU_THIS_PTR msr.kernelgsbase = context->kernel_gs_base.Reg64;
  BX_CPU_THIS_PTR msr.apicbase = context->apic_base.Reg64;
  BX_CPU_THIS_PTR msr.pat._u64 = context->pat.Reg64;
  BX_CPU_THIS_PTR msr.sysenter_cs_msr = context->sysenter_cs.Reg32;
  BX_CPU_THIS_PTR msr.sysenter_eip_msr = context->sysenter_eip.Reg64;
  BX_CPU_THIS_PTR msr.sysenter_esp_msr = context->sysenter_esp.Reg64;
  BX_CPU_THIS_PTR msr.star = context->star.Reg64;
  BX_CPU_THIS_PTR msr.lstar = context->lstar.Reg64;
  BX_CPU_THIS_PTR msr.cstar = context->cstar.Reg64;
  BX_CPU_THIS_PTR msr.fmask = context->sfmask.Reg32;
  BX_CPU_THIS_PTR msr.tsc_aux = context->tsc_aux.Reg32;

  // The next few lines are taken from the mov cr0 implementation and attempt
  // to make sure Bochs updates internal state depending on if mode changes
  // occured from the newly commit register state

#if BX_CPU_LEVEL >= 4
  BX_CPU_THIS_PTR handleAlignmentCheck(/* CR0.AC reloaded */);
#endif

  BX_CPU_THIS_PTR handleCpuModeChange();

#if BX_CPU_LEVEL >= 6
  BX_CPU_THIS_PTR handleSseModeChange();
#if BX_SUPPORT_AVX
  BX_CPU_THIS_PTR handleAvxModeChange();
#endif
#endif
}

// Segment getter helper
#define GET_SEGMENT_FULL(name, bochs_seg) \
  context->name.Segment.Base       = bochs_seg.cache.u.segment.base;\
  context->name.Segment.Limit      = bochs_seg.cache.u.segment.limit_scaled;\
  context->name.Segment.Selector   = bochs_seg.selector.value;\
  context->name.Segment.Attributes = (BX_CPU_THIS_PTR get_descriptor_h(&bochs_seg.cache) >> 8) & 0xffff;

// Floating point register value getter
#define GET_FP_REG(name, bochs_idx) \
  context->name.Fp.Mantissa       = BX_READ_FPU_REG(bochs_idx).fraction;\
  context->name.Fp.BiasedExponent = BX_READ_FPU_REG(bochs_idx).exp & 0x7fff;\
  context->name.Fp.Sign           = (BX_READ_FPU_REG(bochs_idx).exp >> 15) & 1;

// get_context implementation to allow Rust to get access to all of the CPU
// state internal to Bochs
void get_context(struct _whvp_context* context) {
  context->rax.Reg64 = RAX;
  context->rcx.Reg64 = RCX;
  context->rdx.Reg64 = RDX;
  context->rbx.Reg64 = RBX;
  context->rsp.Reg64 = RSP;
  context->rbp.Reg64 = RBP;
  context->rsi.Reg64 = RSI;
  context->rdi.Reg64 = RDI;
  context->r8.Reg64  = R8;
  context->r9.Reg64  = R9;
  context->r10.Reg64 = R10;
  context->r11.Reg64 = R11;
  context->r12.Reg64 = R12;
  context->r13.Reg64 = R13;
  context->r14.Reg64 = R14;
  context->r15.Reg64 = R15;
  context->rip.Reg64 = RIP;
  context->rflags.Reg64 = BX_CPU_THIS_PTR read_eflags();

  GET_SEGMENT_FULL(es, BX_CPU_THIS_PTR sregs[BX_SEG_REG_ES]);
  GET_SEGMENT_FULL(cs, BX_CPU_THIS_PTR sregs[BX_SEG_REG_CS]);
  GET_SEGMENT_FULL(ss, BX_CPU_THIS_PTR sregs[BX_SEG_REG_SS]);
  GET_SEGMENT_FULL(ds, BX_CPU_THIS_PTR sregs[BX_SEG_REG_DS]);
  GET_SEGMENT_FULL(fs, BX_CPU_THIS_PTR sregs[BX_SEG_REG_FS]);
  GET_SEGMENT_FULL(gs, BX_CPU_THIS_PTR sregs[BX_SEG_REG_GS]);

  GET_SEGMENT_FULL(ldtr, BX_CPU_THIS_PTR ldtr);
  GET_SEGMENT_FULL(tr, BX_CPU_THIS_PTR tr);
  context->idtr.Table.Base  = BX_CPU_THIS_PTR idtr.base;
  context->idtr.Table.Limit = BX_CPU_THIS_PTR idtr.limit;
  context->gdtr.Table.Base  = BX_CPU_THIS_PTR gdtr.base;
  context->gdtr.Table.Limit = BX_CPU_THIS_PTR gdtr.limit;

  context->cr0.Reg64 = BX_CPU_THIS_PTR cr0.get32();
  context->cr2.Reg64 = BX_CPU_THIS_PTR cr2;
  context->cr3.Reg64 = BX_CPU_THIS_PTR cr3;
  context->cr4.Reg64 = BX_CPU_THIS_PTR cr4.get32();
  context->cr8.Reg64 = BX_CPU_THIS_PTR get_cr8();

  context->dr0.Reg64 = BX_CPU_THIS_PTR dr[0];
  context->dr1.Reg64 = BX_CPU_THIS_PTR dr[1];
  context->dr2.Reg64 = BX_CPU_THIS_PTR dr[2];
  context->dr3.Reg64 = BX_CPU_THIS_PTR dr[3];
  context->dr6.Reg64 = BX_CPU_THIS_PTR dr6.get32();
  context->dr7.Reg64 = BX_CPU_THIS_PTR dr7.get32();

  context->xcr0.Reg64 = BX_CPU_THIS_PTR xcr0.get32();

  memcpy(context->xmm0.Reg128.Dword, BX_READ_XMM_REG(0).xmm_u32, 16);
  memcpy(context->xmm1.Reg128.Dword, BX_READ_XMM_REG(1).xmm_u32, 16);
  memcpy(context->xmm2.Reg128.Dword, BX_READ_XMM_REG(2).xmm_u32, 16);
  memcpy(context->xmm3.Reg128.Dword, BX_READ_XMM_REG(3).xmm_u32, 16);
  memcpy(context->xmm4.Reg128.Dword, BX_READ_XMM_REG(4).xmm_u32, 16);
  memcpy(context->xmm5.Reg128.Dword, BX_READ_XMM_REG(5).xmm_u32, 16);
  memcpy(context->xmm6.Reg128.Dword, BX_READ_XMM_REG(6).xmm_u32, 16);
  memcpy(context->xmm7.Reg128.Dword, BX_READ_XMM_REG(7).xmm_u32, 16);
  memcpy(context->xmm8.Reg128.Dword, BX_READ_XMM_REG(8).xmm_u32, 16);
  memcpy(context->xmm9.Reg128.Dword, BX_READ_XMM_REG(9).xmm_u32, 16);
  memcpy(context->xmm10.Reg128.Dword, BX_READ_XMM_REG(10).xmm_u32, 16);
  memcpy(context->xmm11.Reg128.Dword, BX_READ_XMM_REG(11).xmm_u32, 16);
  memcpy(context->xmm12.Reg128.Dword, BX_READ_XMM_REG(12).xmm_u32, 16);
  memcpy(context->xmm13.Reg128.Dword, BX_READ_XMM_REG(13).xmm_u32, 16);
  memcpy(context->xmm14.Reg128.Dword, BX_READ_XMM_REG(14).xmm_u32, 16);
  memcpy(context->xmm15.Reg128.Dword, BX_READ_XMM_REG(15).xmm_u32, 16);

  GET_FP_REG(st0, 0);
  GET_FP_REG(st1, 1);
  GET_FP_REG(st2, 2);
  GET_FP_REG(st3, 3);
  GET_FP_REG(st4, 4);
  GET_FP_REG(st5, 5);
  GET_FP_REG(st6, 6);
  GET_FP_REG(st7, 7);

  context->fp_control.FpControlStatus.FpControl = BX_CPU_THIS_PTR the_i387.get_control_word();
  context->fp_control.FpControlStatus.FpStatus  = BX_CPU_THIS_PTR the_i387.get_status_word();
  context->fp_control.FpControlStatus.FpTag     = (Bit8u)BX_CPU_THIS_PTR the_i387.get_tag_word();
  context->fp_control.FpControlStatus.LastFpOp  = BX_CPU_THIS_PTR the_i387.foo;

  if(BX_CPU_THIS_PTR efer.get_LMA()) {
    // Long mode state
    context->fp_control.FpControlStatus.LastFpRip = BX_CPU_THIS_PTR the_i387.fip;
  } else {
    // Other mode state
    context->fp_control.FpControlStatus.LastFpEip = (Bit32u)BX_CPU_THIS_PTR the_i387.fip;
    context->fp_control.FpControlStatus.LastFpCs  = BX_CPU_THIS_PTR the_i387.fcs;
  }

  context->xmm_control.XmmControlStatus.XmmStatusControl     = BX_CPU_THIS_PTR mxcsr.mxcsr;
  context->xmm_control.XmmControlStatus.XmmStatusControlMask = BX_CPU_THIS_PTR mxcsr_mask;

  if(BX_CPU_THIS_PTR efer.get_LMA()) {
    // Long mode state
    context->xmm_control.XmmControlStatus.LastFpRdp = BX_CPU_THIS_PTR the_i387.fdp;
  } else {
    // Other mode state
    context->xmm_control.XmmControlStatus.LastFpDp = (Bit32u)BX_CPU_THIS_PTR the_i387.fdp;
    context->xmm_control.XmmControlStatus.LastFpDs  = BX_CPU_THIS_PTR the_i387.fds;
  }

  context->tsc.Reg64  = BX_CPU_THIS_PTR get_TSC();
  context->efer.Reg64 = BX_CPU_THIS_PTR efer.get32();
  context->kernel_gs_base.Reg64 = BX_CPU_THIS_PTR msr.kernelgsbase;
  //context->apic_base.Reg64 = BX_CPU_THIS_PTR msr.apicbase;
  context->pat.Reg64 = BX_CPU_THIS_PTR msr.pat._u64;
  context->sysenter_cs.Reg64 = BX_CPU_THIS_PTR msr.sysenter_cs_msr;
  context->sysenter_eip.Reg64 = BX_CPU_THIS_PTR msr.sysenter_eip_msr;
  context->sysenter_esp.Reg64 = BX_CPU_THIS_PTR msr.sysenter_esp_msr;
  context->star.Reg64 = BX_CPU_THIS_PTR msr.star;
  context->lstar.Reg64 = BX_CPU_THIS_PTR msr.lstar;
  context->cstar.Reg64 = BX_CPU_THIS_PTR msr.cstar;
  context->sfmask.Reg64 = BX_CPU_THIS_PTR msr.fmask;
  context->tsc_aux.Reg64 = BX_CPU_THIS_PTR msr.tsc_aux;
}

// step_cpu() implementation which allows Rust to run a certain amount of
// instructions (or chains with optimizations on).
//
// This code is nearly directly copied and pasted from the actual Bochs CPU
// loop
void step_cpu(Bit64u steps) {
  // Flush data TLBs, this might not be needed but we do it anyways
  BX_CPU_THIS_PTR TLB_flush();

  // Step while we have steps... duh
  while(steps) {
    // Completed a step
    steps--;
    
    // check on events which occurred for previous instructions (traps)
    // and ones which are asynchronous to the CPU (hardware interrupts)
    if (BX_CPU_THIS_PTR async_event) {
      if (BX_CPU_THIS_PTR handleAsyncEvent()) {
        // If request to return to caller ASAP.
        return;
      }
    }

    bxICacheEntry_c *entry = BX_CPU_THIS_PTR getICacheEntry();
    bxInstruction_c *i = entry->i;

#if BX_SUPPORT_HANDLERS_CHAINING_SPEEDUPS
    {
      // want to allow changing of the instruction inside instrumentation callback
      BX_INSTR_BEFORE_EXECUTION(BX_CPU_ID, i);
      RIP += i->ilen();
      // when handlers chaining is enabled this single call will execute entire trace
      BX_CPU_CALL_METHOD(i->execute1, (i)); // might iterate repeat instruction
      BX_SYNC_TIME_IF_SINGLE_PROCESSOR(0);

      if (BX_CPU_THIS_PTR async_event) continue;

      i = BX_CPU_THIS_PTR getICacheEntry()->i;
    }
#else // BX_SUPPORT_HANDLERS_CHAINING_SPEEDUPS == 0

    bxInstruction_c *last = i + (entry->tlen);

    {

#if BX_DEBUGGER
      if (BX_CPU_THIS_PTR trace)
        debug_disasm_instruction(BX_CPU_THIS_PTR prev_rip);
#endif

      // want to allow changing of the instruction inside instrumentation callback
      BX_INSTR_BEFORE_EXECUTION(BX_CPU_ID, i);
      RIP += i->ilen();
      BX_CPU_CALL_METHOD(i->execute1, (i)); // might iterate repeat instruction
      BX_CPU_THIS_PTR prev_rip = RIP; // commit new RIP
      BX_INSTR_AFTER_EXECUTION(BX_CPU_ID, i);
      BX_CPU_THIS_PTR icount++;

      BX_SYNC_TIME_IF_SINGLE_PROCESSOR(0);

      // note instructions generating exceptions never reach this point
#if BX_DEBUGGER || BX_GDBSTUB
      if (dbg_instruction_epilog()) return;
#endif

      if (BX_CPU_THIS_PTR async_event) continue;

      if (++i == last) {
        entry = BX_CPU_THIS_PTR getICacheEntry();
        i = entry->i;
        last = i + (entry->tlen);
      }
    }
#endif

    // clear stop trace magic indication that probably was set by repeat or branch32/64
    BX_CPU_THIS_PTR async_event &= ~BX_ASYNC_EVENT_STOP_TRACE;
  }

  // Flush TLBs again, once again, might not be needed
  BX_CPU_THIS_PTR TLB_flush();
}

// step_device() implementation. This steps the device and time emulation in
// Bochs. This is used very frequently to make sure things like timer interrupts
// are delivered to the guest.
void step_device(Bit64u steps) {
  while(steps) {
    // Check for async events and handle them if there are any
    if (BX_CPU_THIS_PTR async_event) {
      if (BX_CPU_THIS_PTR handleAsyncEvent()) {
        // If request to return to caller ASAP.
        return;
      }
    }

    // We actually tick one at a time even though we could tick in bulk. This
    // allows us to check for async events more frequently and it makes for a
    // lower latency hypervisor experience. This could be tweaked higher for
    // more performance, at the cost of usability.
    //
    // Tuning this further might cause interrupts to get queued up without being
    // handled so it could actually potentially cause corruption in the guest.
    // Be careful changing things like this.
    bx_pc_system.tickn(1);
    steps--;
  }
}

// State that tracks if we've initialized bochservisor
int already_booted = 0;

// Routines passed to Rust
struct _bochs_routines routines = { 0 };

// Cached address of the Rust routine to call instead of the normal CPU loop
void (*bochs_cpu_loop)(struct _bochs_routines*, Bit64u) = NULL;

void BX_CPU_C::cpu_loop(void)
{
#if BX_DEBUGGER
  BX_CPU_THIS_PTR break_point = 0;
  BX_CPU_THIS_PTR magic_break = 0;
  BX_CPU_THIS_PTR stop_reason = STOP_NO_REASON;
#endif

  if (setjmp(BX_CPU_THIS_PTR jmp_buf_env)) {
    // can get here only from exception function or VMEXIT
    BX_CPU_THIS_PTR icount++;
    BX_SYNC_TIME_IF_SINGLE_PROCESSOR(0);
#if BX_DEBUGGER || BX_GDBSTUB
    if (dbg_instruction_epilog()) return;
#endif
#if BX_GDBSTUB
    if (bx_dbg.gdbstub_enabled) return;
#endif
  }

  // If the exception() routine has encountered a nasty fault scenario,
  // the debugger may request that control is returned to it so that
  // the situation may be examined.
#if BX_DEBUGGER
  if (bx_guard.interrupt_requested) return;
#endif

  // We get here either by a normal function call, or by a longjmp
  // back from an exception() call.  In either case, commit the
  // new EIP/ESP, and set up other environmental fields.  This code
  // mirrors similar code below, after the interrupt() call.
  BX_CPU_THIS_PTR prev_rip = RIP; // commit new EIP
  BX_CPU_THIS_PTR speculative_rsp = 0;

  // Check if we've run once before (due to returns and longjumps this gets hit
  // multiple times)
  if(!already_booted) {
    // Enforce IPS is what we expect
    Bit64u ips = SIM->get_param_num(BXPN_IPS)->get();
    if(ips != 1000000) {
      fprintf(stderr, "Bochservisor requires ips=1000000 in your bochsrc!\n");
      exit(-1);
    }

    // We only support single core right now, enforce that
    Bit64u procs   = SIM->get_param_num(BXPN_CPU_NPROCESSORS)->get();
    Bit64u cores   = SIM->get_param_num(BXPN_CPU_NCORES)->get();
    Bit64u threads = SIM->get_param_num(BXPN_CPU_NTHREADS)->get();
    if(procs != 1 || cores != 1 || threads != 1) {
      fprintf(stderr, "Bochservisor requires procs=cores=threads=1 in your bochsrc!\n");
      exit(-1);
    }

    // Make sure clock syncing is set to none
    int clock_sync = SIM->get_param_enum(BXPN_CLOCK_SYNC)->get();
    if(clock_sync != BX_CLOCK_SYNC_NONE) {
      fprintf(stderr, "Bochservisor requires clock: sync=none in your bochsrc!\n");
      exit(-1);
    }

    // Load the bochservisor DLL
    HMODULE module = LoadLibrary("..\\bochservisor\\target\\release\\bochservisor.dll");
    if(!module) {
      fprintf(stderr, "LoadLibrary() error : %d\n", GetLastError());
      exit(-1);
    }

    // Configure the routines to hand to Rust for manipulating Bochs
    routines.set_context        = set_context;
    routines.get_context        = get_context;
    routines.step_device        = step_device;
    routines.step_cpu           = step_cpu;
    routines.get_memory_backing = get_memory_backing;

    // Lookup the address of the Rust CPU look implementation in the DLL
    bochs_cpu_loop = (void (*)(struct _bochs_routines*, Bit64u))
      GetProcAddress(module, "bochs_cpu_loop");
    if(!bochs_cpu_loop) {
      fprintf(stderr, "GetProcAddress() error : %d\n", GetLastError());
      exit(-1);
    }

    // Change state to indicate we've run the initializiation tasks
    already_booted = 1;
  }

  // Jump into the Rust CPU loop implementation
  (*bochs_cpu_loop)(&routines, BX_MEM(0)->get_memory_len());
  return;

  while (1) {
    // check on events which occurred for previous instructions (traps)
    // and ones which are asynchronous to the CPU (hardware interrupts)
    if (BX_CPU_THIS_PTR async_event) {
      if (handleAsyncEvent()) {
        // If request to return to caller ASAP.
        return;
      }
    }

    bxICacheEntry_c *entry = getICacheEntry();
    bxInstruction_c *i = entry->i;

#if BX_SUPPORT_HANDLERS_CHAINING_SPEEDUPS
    for(;;) {
      // want to allow changing of the instruction inside instrumentation callback
      BX_INSTR_BEFORE_EXECUTION(BX_CPU_ID, i);
      RIP += i->ilen();
      // when handlers chaining is enabled this single call will execute entire trace
      BX_CPU_CALL_METHOD(i->execute1, (i)); // might iterate repeat instruction

      BX_SYNC_TIME_IF_SINGLE_PROCESSOR(0);

      if (BX_CPU_THIS_PTR async_event) break;

      i = getICacheEntry()->i;
    }
#else // BX_SUPPORT_HANDLERS_CHAINING_SPEEDUPS == 0

    bxInstruction_c *last = i + (entry->tlen);

    for(;;) {

#if BX_DEBUGGER
      if (BX_CPU_THIS_PTR trace)
        debug_disasm_instruction(BX_CPU_THIS_PTR prev_rip);
#endif

      // want to allow changing of the instruction inside instrumentation callback
      BX_INSTR_BEFORE_EXECUTION(BX_CPU_ID, i);
      RIP += i->ilen();
      BX_CPU_CALL_METHOD(i->execute1, (i)); // might iterate repeat instruction
      BX_CPU_THIS_PTR prev_rip = RIP; // commit new RIP
      BX_INSTR_AFTER_EXECUTION(BX_CPU_ID, i);
      BX_CPU_THIS_PTR icount++;

      BX_SYNC_TIME_IF_SINGLE_PROCESSOR(0);

      // note instructions generating exceptions never reach this point
#if BX_DEBUGGER || BX_GDBSTUB
      if (dbg_instruction_epilog()) return;
#endif

      if (BX_CPU_THIS_PTR async_event) break;

      if (++i == last) {
        entry = getICacheEntry();
        i = entry->i;
        last = i + (entry->tlen);
      }
    }
#endif

    // clear stop trace magic indication that probably was set by repeat or branch32/64
    BX_CPU_THIS_PTR async_event &= ~BX_ASYNC_EVENT_STOP_TRACE;

  }  // while (1)
}

#if BX_SUPPORT_SMP

void BX_CPU_C::cpu_run_trace(void)
{
  if (setjmp(BX_CPU_THIS_PTR jmp_buf_env)) {
    // can get here only from exception function or VMEXIT
    BX_CPU_THIS_PTR icount++;
    return;
  }

  // check on events which occurred for previous instructions (traps)
  // and ones which are asynchronous to the CPU (hardware interrupts)
  if (BX_CPU_THIS_PTR async_event) {
    if (handleAsyncEvent()) {
      // If request to return to caller ASAP.
      return;
    }
  }

  bxICacheEntry_c *entry = getICacheEntry();
  bxInstruction_c *i = entry->i;

#if BX_SUPPORT_HANDLERS_CHAINING_SPEEDUPS
  // want to allow changing of the instruction inside instrumentation callback
  BX_INSTR_BEFORE_EXECUTION(BX_CPU_ID, i);
  RIP += i->ilen();
  // when handlers chaining is enabled this single call will execute entire trace
  BX_CPU_CALL_METHOD(i->execute1, (i)); // might iterate repeat instruction

  if (BX_CPU_THIS_PTR async_event) {
    // clear stop trace magic indication that probably was set by repeat or branch32/64
    BX_CPU_THIS_PTR async_event &= ~BX_ASYNC_EVENT_STOP_TRACE;
  }
#else
  bxInstruction_c *last = i + (entry->tlen);

  for(;;) {
    // want to allow changing of the instruction inside instrumentation callback
    BX_INSTR_BEFORE_EXECUTION(BX_CPU_ID, i);
    RIP += i->ilen();
    BX_CPU_CALL_METHOD(i->execute1, (i)); // might iterate repeat instruction
    BX_CPU_THIS_PTR prev_rip = RIP; // commit new RIP
    BX_INSTR_AFTER_EXECUTION(BX_CPU_ID, i);
    BX_CPU_THIS_PTR icount++;

    if (BX_CPU_THIS_PTR async_event) {
      // clear stop trace magic indication that probably was set by repeat or branch32/64
      BX_CPU_THIS_PTR async_event &= ~BX_ASYNC_EVENT_STOP_TRACE;
      break;
    }

    if (++i == last) break;
  }
#endif // BX_SUPPORT_HANDLERS_CHAINING_SPEEDUPS
}

#endif

bxICacheEntry_c* BX_CPU_C::getICacheEntry(void)
{
  bx_address eipBiased = RIP + BX_CPU_THIS_PTR eipPageBias;

  if (eipBiased >= BX_CPU_THIS_PTR eipPageWindowSize) {
    prefetch();
    eipBiased = RIP + BX_CPU_THIS_PTR eipPageBias;
  }

  INC_ICACHE_STAT(iCacheLookups);

  bx_phy_address pAddr = BX_CPU_THIS_PTR pAddrFetchPage + eipBiased;
  bxICacheEntry_c *entry = NULL; //BX_CPU_THIS_PTR iCache.find_entry(pAddr, BX_CPU_THIS_PTR fetchModeMask);

  if (entry == NULL)
  {
    // iCache miss. No validated instruction with matching fetch parameters
    // is in the iCache.
    INC_ICACHE_STAT(iCacheMisses);
    entry = serveICacheMiss((Bit32u) eipBiased, pAddr);
  }

  return entry;
}

#if BX_SUPPORT_HANDLERS_CHAINING_SPEEDUPS && BX_ENABLE_TRACE_LINKING

// The function is called after taken branch instructions and tries to link the branch to the next trace
void BX_CPP_AttrRegparmN(1) BX_CPU_C::linkTrace(bxInstruction_c *i)
{
#if BX_SUPPORT_SMP
  if (BX_SMP_PROCESSORS > 1)
    return;
#endif

#define BX_HANDLERS_CHAINING_MAX_DEPTH 1000

  // do not allow extreme trace link depth / avoid host stack overflow
  // (could happen with badly compiled instruction handlers)
  static Bit32u linkDepth = 0;

  if (BX_CPU_THIS_PTR async_event || ++linkDepth > BX_HANDLERS_CHAINING_MAX_DEPTH) {
    linkDepth = 0;
    return;
  }

  Bit32u delta = (Bit32u) (BX_CPU_THIS_PTR icount - BX_CPU_THIS_PTR icount_last_sync);
  if(delta >= bx_pc_system.getNumCpuTicksLeftNextEvent()) {
    linkDepth = 0;
    return;
  }

  bxInstruction_c *next = i->getNextTrace(BX_CPU_THIS_PTR iCache.traceLinkTimeStamp);
  if (next) {
    BX_EXECUTE_INSTRUCTION(next);
    return;
  }

  bx_address eipBiased = RIP + BX_CPU_THIS_PTR eipPageBias;
  if (eipBiased >= BX_CPU_THIS_PTR eipPageWindowSize) {
    prefetch();
    eipBiased = RIP + BX_CPU_THIS_PTR eipPageBias;
  }

  INC_ICACHE_STAT(iCacheLookups);

  bx_phy_address pAddr = BX_CPU_THIS_PTR pAddrFetchPage + eipBiased;
  bxICacheEntry_c *entry = BX_CPU_THIS_PTR iCache.find_entry(pAddr, BX_CPU_THIS_PTR fetchModeMask);

  if (entry != NULL) // link traces - handle only hit cases
  {
    i->setNextTrace(entry->i, BX_CPU_THIS_PTR iCache.traceLinkTimeStamp);
    i = entry->i;
    BX_EXECUTE_INSTRUCTION(i);
  }
}

#endif

#define BX_REPEAT_TIME_UPDATE_INTERVAL (BX_MAX_TRACE_LENGTH-1)

void BX_CPP_AttrRegparmN(2) BX_CPU_C::repeat(bxInstruction_c *i, BxRepIterationPtr_tR execute)
{
  // non repeated instruction
  if (! i->repUsedL()) {
    BX_CPU_CALL_REP_ITERATION(execute, (i));
    return;
  }

#if BX_X86_DEBUGGER
  BX_CPU_THIS_PTR in_repeat = 0;
#endif

#if BX_SUPPORT_X86_64
  if (i->as64L()) {
    while(1) {
      if (RCX != 0) {
        BX_CPU_CALL_REP_ITERATION(execute, (i));
        BX_INSTR_REPEAT_ITERATION(BX_CPU_ID, i);
        RCX --;
      }
      if (RCX == 0) return;

#if BX_DEBUGGER == 0
      if (BX_CPU_THIS_PTR async_event)
#endif
        break; // exit always if debugger enabled

      BX_CPU_THIS_PTR icount++;

      BX_SYNC_TIME_IF_SINGLE_PROCESSOR(BX_REPEAT_TIME_UPDATE_INTERVAL);
    }
  }
  else
#endif
  if (i->as32L()) {
    while(1) {
      if (ECX != 0) {
        BX_CPU_CALL_REP_ITERATION(execute, (i));
        BX_INSTR_REPEAT_ITERATION(BX_CPU_ID, i);
        RCX = ECX - 1;
      }
      if (ECX == 0) return;

#if BX_DEBUGGER == 0
      if (BX_CPU_THIS_PTR async_event)
#endif
        break; // exit always if debugger enabled

      BX_CPU_THIS_PTR icount++;

      BX_SYNC_TIME_IF_SINGLE_PROCESSOR(BX_REPEAT_TIME_UPDATE_INTERVAL);
    }
  }
  else  // 16bit addrsize
  {
    while(1) {
      if (CX != 0) {
        BX_CPU_CALL_REP_ITERATION(execute, (i));
        BX_INSTR_REPEAT_ITERATION(BX_CPU_ID, i);
        CX --;
      }
      if (CX == 0) return;

#if BX_DEBUGGER == 0
      if (BX_CPU_THIS_PTR async_event)
#endif
        break; // exit always if debugger enabled

      BX_CPU_THIS_PTR icount++;

      BX_SYNC_TIME_IF_SINGLE_PROCESSOR(BX_REPEAT_TIME_UPDATE_INTERVAL);
    }
  }

#if BX_X86_DEBUGGER
  BX_CPU_THIS_PTR in_repeat = 1;
#endif

  RIP = BX_CPU_THIS_PTR prev_rip; // repeat loop not done, restore RIP

  // assert magic async_event to stop trace execution
  BX_CPU_THIS_PTR async_event |= BX_ASYNC_EVENT_STOP_TRACE;
}

void BX_CPP_AttrRegparmN(2) BX_CPU_C::repeat_ZF(bxInstruction_c *i, BxRepIterationPtr_tR execute)
{
  unsigned rep = i->lockRepUsedValue();

  // non repeated instruction
  if (rep < 2) {
    BX_CPU_CALL_REP_ITERATION(execute, (i));
    return;
  }

#if BX_X86_DEBUGGER
  BX_CPU_THIS_PTR in_repeat = 0;
#endif

  if (rep == 3) { /* repeat prefix 0xF3 */
#if BX_SUPPORT_X86_64
    if (i->as64L()) {
      while(1) {
        if (RCX != 0) {
          BX_CPU_CALL_REP_ITERATION(execute, (i));
          BX_INSTR_REPEAT_ITERATION(BX_CPU_ID, i);
          RCX --;
        }
        if (! get_ZF() || RCX == 0) return;

#if BX_DEBUGGER == 0
        if (BX_CPU_THIS_PTR async_event)
#endif
          break; // exit always if debugger enabled

        BX_CPU_THIS_PTR icount++;

        BX_SYNC_TIME_IF_SINGLE_PROCESSOR(BX_REPEAT_TIME_UPDATE_INTERVAL);
      }
    }
    else
#endif
    if (i->as32L()) {
      while(1) {
        if (ECX != 0) {
          BX_CPU_CALL_REP_ITERATION(execute, (i));
          BX_INSTR_REPEAT_ITERATION(BX_CPU_ID, i);
          RCX = ECX - 1;
        }
        if (! get_ZF() || ECX == 0) return;

#if BX_DEBUGGER == 0
        if (BX_CPU_THIS_PTR async_event)
#endif
          break; // exit always if debugger enabled

        BX_CPU_THIS_PTR icount++;

        BX_SYNC_TIME_IF_SINGLE_PROCESSOR(BX_REPEAT_TIME_UPDATE_INTERVAL);
      }
    }
    else  // 16bit addrsize
    {
      while(1) {
        if (CX != 0) {
          BX_CPU_CALL_REP_ITERATION(execute, (i));
          BX_INSTR_REPEAT_ITERATION(BX_CPU_ID, i);
          CX --;
        }
        if (! get_ZF() || CX == 0) return;

#if BX_DEBUGGER == 0
        if (BX_CPU_THIS_PTR async_event)
#endif
          break; // exit always if debugger enabled

        BX_CPU_THIS_PTR icount++;

        BX_SYNC_TIME_IF_SINGLE_PROCESSOR(BX_REPEAT_TIME_UPDATE_INTERVAL);
      }
    }
  }
  else {          /* repeat prefix 0xF2 */
#if BX_SUPPORT_X86_64
    if (i->as64L()) {
      while(1) {
        if (RCX != 0) {
          BX_CPU_CALL_REP_ITERATION(execute, (i));
          BX_INSTR_REPEAT_ITERATION(BX_CPU_ID, i);
          RCX --;
        }
        if (get_ZF() || RCX == 0) return;

#if BX_DEBUGGER == 0
        if (BX_CPU_THIS_PTR async_event)
#endif
          break; // exit always if debugger enabled

        BX_CPU_THIS_PTR icount++;

        BX_SYNC_TIME_IF_SINGLE_PROCESSOR(BX_REPEAT_TIME_UPDATE_INTERVAL);
      }
    }
    else
#endif
    if (i->as32L()) {
      while(1) {
        if (ECX != 0) {
          BX_CPU_CALL_REP_ITERATION(execute, (i));
          BX_INSTR_REPEAT_ITERATION(BX_CPU_ID, i);
          RCX = ECX - 1;
        }
        if (get_ZF() || ECX == 0) return;

#if BX_DEBUGGER == 0
        if (BX_CPU_THIS_PTR async_event)
#endif
          break; // exit always if debugger enabled

        BX_CPU_THIS_PTR icount++;

        BX_SYNC_TIME_IF_SINGLE_PROCESSOR(BX_REPEAT_TIME_UPDATE_INTERVAL);
      }
    }
    else  // 16bit addrsize
    {
      while(1) {
        if (CX != 0) {
          BX_CPU_CALL_REP_ITERATION(execute, (i));
          BX_INSTR_REPEAT_ITERATION(BX_CPU_ID, i);
          CX --;
        }
        if (get_ZF() || CX == 0) return;

#if BX_DEBUGGER == 0
        if (BX_CPU_THIS_PTR async_event)
#endif
          break; // exit always if debugger enabled

        BX_CPU_THIS_PTR icount++;

        BX_SYNC_TIME_IF_SINGLE_PROCESSOR(BX_REPEAT_TIME_UPDATE_INTERVAL);
      }
    }
  }

#if BX_X86_DEBUGGER
  BX_CPU_THIS_PTR in_repeat = 1;
#endif

  RIP = BX_CPU_THIS_PTR prev_rip; // repeat loop not done, restore RIP

  // assert magic async_event to stop trace execution
  BX_CPU_THIS_PTR async_event |= BX_ASYNC_EVENT_STOP_TRACE;
}

// boundaries of consideration:
//
//  * physical memory boundary: 1024k (1Megabyte) (increments of...)
//  * A20 boundary:             1024k (1Megabyte)
//  * page boundary:            4k
//  * ROM boundary:             2k (dont care since we are only reading)
//  * segment boundary:         any

void BX_CPU_C::prefetch(void)
{
  bx_address laddr;
  unsigned pageOffset;

  INC_ICACHE_STAT(iCachePrefetch);

#if BX_SUPPORT_X86_64
  if (long64_mode()) {
    if (! IsCanonical(RIP)) {
      BX_ERROR(("prefetch: #GP(0): RIP crossed canonical boundary"));
      exception(BX_GP_EXCEPTION, 0);
    }

    // linear address is equal to RIP in 64-bit long mode
    pageOffset = PAGE_OFFSET(EIP);
    laddr = RIP;

    // Calculate RIP at the beginning of the page.
    BX_CPU_THIS_PTR eipPageBias = pageOffset - RIP;
    BX_CPU_THIS_PTR eipPageWindowSize = 4096;
  }
  else
#endif
  {

#if BX_CPU_LEVEL >= 5
    if (USER_PL && BX_CPU_THIS_PTR get_VIP() && BX_CPU_THIS_PTR get_VIF()) {
      if (BX_CPU_THIS_PTR cr4.get_PVI() | (v8086_mode() && BX_CPU_THIS_PTR cr4.get_VME())) {
        BX_ERROR(("prefetch: inconsistent VME state"));
        exception(BX_GP_EXCEPTION, 0);
      }
    }
#endif

    BX_CLEAR_64BIT_HIGH(BX_64BIT_REG_RIP); /* avoid 32-bit EIP wrap */
    laddr = get_laddr32(BX_SEG_REG_CS, EIP);
    pageOffset = PAGE_OFFSET(laddr);

    // Calculate RIP at the beginning of the page.
    BX_CPU_THIS_PTR eipPageBias = (bx_address) pageOffset - EIP;

    Bit32u limit = BX_CPU_THIS_PTR sregs[BX_SEG_REG_CS].cache.u.segment.limit_scaled;
    if (EIP > limit) {
      BX_ERROR(("prefetch: EIP [%08x] > CS.limit [%08x]", EIP, limit));
      exception(BX_GP_EXCEPTION, 0);
    }

    BX_CPU_THIS_PTR eipPageWindowSize = 4096;
    if (limit + BX_CPU_THIS_PTR eipPageBias < 4096) {
      BX_CPU_THIS_PTR eipPageWindowSize = (Bit32u)(limit + BX_CPU_THIS_PTR eipPageBias + 1);
    }
  }

#if BX_X86_DEBUGGER
  if (hwbreakpoint_check(laddr, BX_HWDebugInstruction, BX_HWDebugInstruction)) {
    signal_event(BX_EVENT_CODE_BREAKPOINT_ASSIST);
    if (! interrupts_inhibited(BX_INHIBIT_DEBUG)) {
       // The next instruction could already hit a code breakpoint but
       // async_event won't take effect immediatelly.
       // Check if the next executing instruction hits code breakpoint

       // check only if not fetching page cross instruction
       // this check is 32-bit wrap safe as well
       if (EIP == (Bit32u) BX_CPU_THIS_PTR prev_rip) {
         Bit32u dr6_bits = code_breakpoint_match(laddr);
         if (dr6_bits & BX_DEBUG_TRAP_HIT) {
           BX_ERROR(("#DB: x86 code breakpoint catched"));
           BX_CPU_THIS_PTR debug_trap |= dr6_bits;
           exception(BX_DB_EXCEPTION, 0);
         }
       }
    }
  }
  else {
    clear_event(BX_EVENT_CODE_BREAKPOINT_ASSIST);
  }
#endif

  BX_CPU_THIS_PTR clear_RF();

  bx_address lpf = LPFOf(laddr);
  bx_TLB_entry *tlbEntry = BX_TLB_ENTRY_OF(laddr, 0);
  Bit8u *fetchPtr = 0;

  if ((tlbEntry->lpf == lpf) && (tlbEntry->accessBits & (0x10 << USER_PL)) != 0) {
    BX_CPU_THIS_PTR pAddrFetchPage = tlbEntry->ppf;
    fetchPtr = (Bit8u*) tlbEntry->hostPageAddr;
  }  
  else {
    bx_phy_address pAddr = translate_linear(tlbEntry, laddr, USER_PL, BX_EXECUTE);
    BX_CPU_THIS_PTR pAddrFetchPage = PPFOf(pAddr);
  }

  if (fetchPtr) {
    BX_CPU_THIS_PTR eipFetchPtr = fetchPtr;
  }
  else {
    BX_CPU_THIS_PTR eipFetchPtr = (const Bit8u*) getHostMemAddr(BX_CPU_THIS_PTR pAddrFetchPage, BX_EXECUTE);

    // Sanity checks
    if (! BX_CPU_THIS_PTR eipFetchPtr) {
      bx_phy_address pAddr = BX_CPU_THIS_PTR pAddrFetchPage + pageOffset;
      if (pAddr >= BX_MEM(0)->get_memory_len()) {
        BX_PANIC(("prefetch: running in bogus memory, pAddr=0x" FMT_PHY_ADDRX, pAddr));
      }
      else {
        BX_PANIC(("prefetch: getHostMemAddr vetoed direct read, pAddr=0x" FMT_PHY_ADDRX, pAddr));
      }
    }
  }
}

#if BX_DEBUGGER || BX_GDBSTUB
bx_bool BX_CPU_C::dbg_instruction_epilog(void)
{
#if BX_DEBUGGER
  bx_address debug_eip = RIP;
  Bit16u cs = BX_CPU_THIS_PTR sregs[BX_SEG_REG_CS].selector.value;

  BX_CPU_THIS_PTR guard_found.cs  = cs;
  BX_CPU_THIS_PTR guard_found.eip = debug_eip;
  BX_CPU_THIS_PTR guard_found.laddr = get_laddr(BX_SEG_REG_CS, debug_eip);
  BX_CPU_THIS_PTR guard_found.code_32_64 = BX_CPU_THIS_PTR fetchModeMask;

  //
  // Take care of break point conditions generated during instruction execution
  //

  // Check if we hit read/write or time breakpoint
  if (BX_CPU_THIS_PTR break_point) {
    Bit64u tt = bx_pc_system.time_ticks();
    switch (BX_CPU_THIS_PTR break_point) {
    case BREAK_POINT_TIME:
      BX_INFO(("[" FMT_LL "d] Caught time breakpoint", tt));
      BX_CPU_THIS_PTR stop_reason = STOP_TIME_BREAK_POINT;
      return(1); // on a breakpoint
    case BREAK_POINT_READ:
      BX_INFO(("[" FMT_LL "d] Caught read watch point", tt));
      BX_CPU_THIS_PTR stop_reason = STOP_READ_WATCH_POINT;
      return(1); // on a breakpoint
    case BREAK_POINT_WRITE:
      BX_INFO(("[" FMT_LL "d] Caught write watch point", tt));
      BX_CPU_THIS_PTR stop_reason = STOP_WRITE_WATCH_POINT;
      return(1); // on a breakpoint
    default:
      BX_PANIC(("Weird break point condition"));
    }
  }

  if (BX_CPU_THIS_PTR magic_break) {
    BX_INFO(("[" FMT_LL "d] Stopped on MAGIC BREAKPOINT", bx_pc_system.time_ticks()));
    BX_CPU_THIS_PTR stop_reason = STOP_MAGIC_BREAK_POINT;
    return(1); // on a breakpoint
  }

  // see if debugger requesting icount guard 
  if (bx_guard.guard_for & BX_DBG_GUARD_ICOUNT) {
    if (get_icount() >= BX_CPU_THIS_PTR guard_found.icount_max) {
      return(1);
    }
  }

  // convenient point to see if user requested debug break or typed Ctrl-C
  if (bx_guard.interrupt_requested) {
    return(1);
  }

  // support for 'show' command in debugger
  extern unsigned dbg_show_mask;
  if(dbg_show_mask) {
    int rv = bx_dbg_show_symbolic();
    if (rv) return(rv);
  }

  // Just committed an instruction, before fetching a new one
  // see if debugger is looking for iaddr breakpoint of any type
  if (bx_guard.guard_for & BX_DBG_GUARD_IADDR_ALL) {
#if (BX_DBG_MAX_VIR_BPOINTS > 0)
    if (bx_guard.guard_for & BX_DBG_GUARD_IADDR_VIR) {
      for (unsigned n=0; n<bx_guard.iaddr.num_virtual; n++) {
        if (bx_guard.iaddr.vir[n].enabled &&
           (bx_guard.iaddr.vir[n].cs  == cs) &&
           (bx_guard.iaddr.vir[n].eip == debug_eip))
        {
          if (! bx_guard.iaddr.vir[n].condition || bx_dbg_eval_condition(bx_guard.iaddr.vir[n].condition)) {
            BX_CPU_THIS_PTR guard_found.guard_found = BX_DBG_GUARD_IADDR_VIR;
            BX_CPU_THIS_PTR guard_found.iaddr_index = n;
            return(1); // on a breakpoint
          }
        }
      }
    }
#endif
#if (BX_DBG_MAX_LIN_BPOINTS > 0)
    if (bx_guard.guard_for & BX_DBG_GUARD_IADDR_LIN) {
      for (unsigned n=0; n<bx_guard.iaddr.num_linear; n++) {
        if (bx_guard.iaddr.lin[n].enabled &&
           (bx_guard.iaddr.lin[n].addr == BX_CPU_THIS_PTR guard_found.laddr))
        {
          if (! bx_guard.iaddr.lin[n].condition || bx_dbg_eval_condition(bx_guard.iaddr.lin[n].condition)) {
            BX_CPU_THIS_PTR guard_found.guard_found = BX_DBG_GUARD_IADDR_LIN;
            BX_CPU_THIS_PTR guard_found.iaddr_index = n;
            return(1); // on a breakpoint
          }
        }
      }
    }
#endif
#if (BX_DBG_MAX_PHY_BPOINTS > 0)
    if (bx_guard.guard_for & BX_DBG_GUARD_IADDR_PHY) {
      bx_phy_address phy;
      bx_bool valid = dbg_xlate_linear2phy(BX_CPU_THIS_PTR guard_found.laddr, &phy);
      if (valid) {
        for (unsigned n=0; n<bx_guard.iaddr.num_physical; n++) {
          if (bx_guard.iaddr.phy[n].enabled && (bx_guard.iaddr.phy[n].addr == phy))
          {
            if (! bx_guard.iaddr.phy[n].condition || bx_dbg_eval_condition(bx_guard.iaddr.phy[n].condition)) {
              BX_CPU_THIS_PTR guard_found.guard_found = BX_DBG_GUARD_IADDR_PHY;
              BX_CPU_THIS_PTR guard_found.iaddr_index = n;
              return(1); // on a breakpoint
            }
          }
        }
      }
    }
#endif
  }
#endif

#if BX_GDBSTUB
  if (bx_dbg.gdbstub_enabled) {
    unsigned reason = bx_gdbstub_check(EIP);
    if (reason != GDBSTUB_STOP_NO_REASON) return(1);
  }
#endif

  return(0);
}
#endif // BX_DEBUGGER || BX_GDBSTUB
