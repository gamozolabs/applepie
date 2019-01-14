use cpu;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};

static FAST_INT_HANDLER: AtomicUsize = AtomicUsize::new(0);

#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
pub struct Idtr {
	limit: u16,
	base:  u64,
}

#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
struct IdtEntry {
	offset_15_0:  u16,
	selector:     u16,
	ist:          u8,
	typ:          u8,
	offset_31_16: u16,
	offset_63_32: u32,
	reserved:     u32,
}

/* pic_init()
 *
 * Summary:
 *
 * This function initializes the PIC. This can only be called on the BSP once.
 * It must be called before init_interrupts().
 */
fn pic_init()
{
    unsafe {
        /* Remap the PIC */
        cpu::out8(0x20, 0x11);
        cpu::out8(0xa0, 0x11);
        cpu::out8(0x21, 0x20);
        cpu::out8(0xa1, 0x30);
        cpu::out8(0x21, 0x04);
        cpu::out8(0xa1, 0x02);
        cpu::out8(0x21, 0x01);
        cpu::out8(0xa1, 0x01);
        
        /* Mask all interrupts on master and slave */
        cpu::out8(0x21, 0xff);
        cpu::out8(0xa1, 0xff);
    }
}

pub fn init_interrupts() {
	let handlers = [
		vec_interrupt_0,  vec_interrupt_1,  vec_interrupt_2,
		vec_interrupt_3,  vec_interrupt_4,  vec_interrupt_5,
		vec_interrupt_6,  vec_interrupt_7,  vec_interrupt_8,
		vec_interrupt_9,  vec_interrupt_10,  vec_interrupt_11,
		vec_interrupt_12,  vec_interrupt_13,  vec_interrupt_14,
		vec_interrupt_15,  vec_interrupt_16,  vec_interrupt_17,
		vec_interrupt_18,  vec_interrupt_19,  vec_interrupt_20,
		vec_interrupt_21,  vec_interrupt_22,  vec_interrupt_23,
		vec_interrupt_24,  vec_interrupt_25,  vec_interrupt_26,
		vec_interrupt_27,  vec_interrupt_28,  vec_interrupt_29,
		vec_interrupt_30,  vec_interrupt_31,  vec_interrupt_32,
		vec_interrupt_33,  vec_interrupt_34,  vec_interrupt_35,
		vec_interrupt_36,  vec_interrupt_37,  vec_interrupt_38,
		vec_interrupt_39,  vec_interrupt_40,  vec_interrupt_41,
		vec_interrupt_42,  vec_interrupt_43,  vec_interrupt_44,
		vec_interrupt_45,  vec_interrupt_46,  vec_interrupt_47,
		vec_interrupt_48,  vec_interrupt_49,  vec_interrupt_50,
		vec_interrupt_51,  vec_interrupt_52,  vec_interrupt_53,
		vec_interrupt_54,  vec_interrupt_55,  vec_interrupt_56,
		vec_interrupt_57,  vec_interrupt_58,  vec_interrupt_59,
		vec_interrupt_60,  vec_interrupt_61,  vec_interrupt_62,
		vec_interrupt_63,  vec_interrupt_64,  vec_interrupt_65,
		vec_interrupt_66,  vec_interrupt_67,  vec_interrupt_68,
		vec_interrupt_69,  vec_interrupt_70,  vec_interrupt_71,
		vec_interrupt_72,  vec_interrupt_73,  vec_interrupt_74,
		vec_interrupt_75,  vec_interrupt_76,  vec_interrupt_77,
		vec_interrupt_78,  vec_interrupt_79,  vec_interrupt_80,
		vec_interrupt_81,  vec_interrupt_82,  vec_interrupt_83,
		vec_interrupt_84,  vec_interrupt_85,  vec_interrupt_86,
		vec_interrupt_87,  vec_interrupt_88,  vec_interrupt_89,
		vec_interrupt_90,  vec_interrupt_91,  vec_interrupt_92,
		vec_interrupt_93,  vec_interrupt_94,  vec_interrupt_95,
		vec_interrupt_96,  vec_interrupt_97,  vec_interrupt_98,
		vec_interrupt_99,  vec_interrupt_100,  vec_interrupt_101,
		vec_interrupt_102,  vec_interrupt_103,  vec_interrupt_104,
		vec_interrupt_105,  vec_interrupt_106,  vec_interrupt_107,
		vec_interrupt_108,  vec_interrupt_109,  vec_interrupt_110,
		vec_interrupt_111,  vec_interrupt_112,  vec_interrupt_113,
		vec_interrupt_114,  vec_interrupt_115,  vec_interrupt_116,
		vec_interrupt_117,  vec_interrupt_118,  vec_interrupt_119,
		vec_interrupt_120,  vec_interrupt_121,  vec_interrupt_122,
		vec_interrupt_123,  vec_interrupt_124,  vec_interrupt_125,
		vec_interrupt_126,  vec_interrupt_127,  vec_interrupt_128,
		vec_interrupt_129,  vec_interrupt_130,  vec_interrupt_131,
		vec_interrupt_132,  vec_interrupt_133,  vec_interrupt_134,
		vec_interrupt_135,  vec_interrupt_136,  vec_interrupt_137,
		vec_interrupt_138,  vec_interrupt_139,  vec_interrupt_140,
		vec_interrupt_141,  vec_interrupt_142,  vec_interrupt_143,
		vec_interrupt_144,  vec_interrupt_145,  vec_interrupt_146,
		vec_interrupt_147,  vec_interrupt_148,  vec_interrupt_149,
		vec_interrupt_150,  vec_interrupt_151,  vec_interrupt_152,
		vec_interrupt_153,  vec_interrupt_154,  vec_interrupt_155,
		vec_interrupt_156,  vec_interrupt_157,  vec_interrupt_158,
		vec_interrupt_159,  vec_interrupt_160,  vec_interrupt_161,
		vec_interrupt_162,  vec_interrupt_163,  vec_interrupt_164,
		vec_interrupt_165,  vec_interrupt_166,  vec_interrupt_167,
		vec_interrupt_168,  vec_interrupt_169,  vec_interrupt_170,
		vec_interrupt_171,  vec_interrupt_172,  vec_interrupt_173,
		vec_interrupt_174,  vec_interrupt_175,  vec_interrupt_176,
		vec_interrupt_177,  vec_interrupt_178,  vec_interrupt_179,
		vec_interrupt_180,  vec_interrupt_181,  vec_interrupt_182,
		vec_interrupt_183,  vec_interrupt_184,  vec_interrupt_185,
		vec_interrupt_186,  vec_interrupt_187,  vec_interrupt_188,
		vec_interrupt_189,  vec_interrupt_190,  vec_interrupt_191,
		vec_interrupt_192,  vec_interrupt_193,  vec_interrupt_194,
		vec_interrupt_195,  vec_interrupt_196,  vec_interrupt_197,
		vec_interrupt_198,  vec_interrupt_199,  vec_interrupt_200,
		vec_interrupt_201,  vec_interrupt_202,  vec_interrupt_203,
		vec_interrupt_204,  vec_interrupt_205,  vec_interrupt_206,
		vec_interrupt_207,  vec_interrupt_208,  vec_interrupt_209,
		vec_interrupt_210,  vec_interrupt_211,  vec_interrupt_212,
		vec_interrupt_213,  vec_interrupt_214,  vec_interrupt_215,
		vec_interrupt_216,  vec_interrupt_217,  vec_interrupt_218,
		vec_interrupt_219,  vec_interrupt_220,  vec_interrupt_221,
		vec_interrupt_222,  vec_interrupt_223,  vec_interrupt_224,
		vec_interrupt_225,  vec_interrupt_226,  vec_interrupt_227,
		vec_interrupt_228,  vec_interrupt_229,  vec_interrupt_230,
		vec_interrupt_231,  vec_interrupt_232,  vec_interrupt_233,
		vec_interrupt_234,  vec_interrupt_235,  vec_interrupt_236,
		vec_interrupt_237,  vec_interrupt_238,  vec_interrupt_239,
		vec_interrupt_240,  vec_interrupt_241,  vec_interrupt_242,
		vec_interrupt_243,  vec_interrupt_244,  vec_interrupt_245,
		vec_interrupt_246,  vec_interrupt_247,  vec_interrupt_248,
		vec_interrupt_249,  vec_interrupt_250,  vec_interrupt_251,
		vec_interrupt_252,  vec_interrupt_253,  vec_interrupt_254,
		vec_interrupt_255,
	];

    print!("Initializing the PIC\n");
    pic_init();

    /* Allocate an IDT and IDTR */
	let mut idt: Vec<IdtEntry> = vec![Default::default(); 256];

	let idtr = Idtr{ limit: 4095, base: idt.as_ptr() as u64 };

	/* Create an entry for each interrupt handler */
	for idt_ent in 0..256 {
		let func = handlers[idt_ent] as usize;

		idt[idt_ent].offset_15_0  = func as u16;
		idt[idt_ent].selector     = 8;
		idt[idt_ent].ist          = 0;
		idt[idt_ent].typ          = 0xee;
		idt[idt_ent].offset_31_16 = (func >> 16) as u16;
		idt[idt_ent].offset_63_32 = (func >> 32) as u32;
		idt[idt_ent].reserved     = 0;
	}

	/* Load the IDT */
	unsafe { cpu::lidt(&idtr as *const _ as *const u8); }

    print!("Interrupt handlers installed\n");
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct InterruptFrame {
	rip:    usize,
	cs:     usize,
	rflags: usize,
	rsp:    usize,
	ss:     usize,
}

pub unsafe fn install_fast_int_handler(handler: usize) {
    print!("Installing fast interrupt handler to 0x{:x}\n", handler);
    FAST_INT_HANDLER.store(handler, Ordering::SeqCst);
}

#[no_mangle]
pub extern fn interrupt_handler(vec: usize, frame: &mut InterruptFrame,
                                error_code: usize) {
    let cr2 = cpu::read_cr2();

    let fast_handler = FAST_INT_HANDLER.load(Ordering::SeqCst);
    if fast_handler > 0 {
        frame.rip = fast_handler;
        return;
    }

    print!(
        "\n\0\x0c=== EXCEPTION =======================================\n\
        Unhandled EXCEPTION {} on core {}\n\
        pc     {:04x}:{:016x}\n\
        stack  {:04x}:{:016x}\n\
        rflags {:016x}\n\
        cr2    {:016x}\n\
        error  {:016x}\n",
        vec, 0,
        {frame.cs}, {frame.rip},
        {frame.ss}, {frame.rsp},
        {frame.rflags},
        cr2,
        error_code);

    panic!("See ya later");
}

extern {
	fn vec_interrupt_0();
	fn vec_interrupt_1();
	fn vec_interrupt_2();
	fn vec_interrupt_3();
	fn vec_interrupt_4();
	fn vec_interrupt_5();
	fn vec_interrupt_6();
	fn vec_interrupt_7();
	fn vec_interrupt_8();
	fn vec_interrupt_9();
	fn vec_interrupt_10();
	fn vec_interrupt_11();
	fn vec_interrupt_12();
	fn vec_interrupt_13();
	fn vec_interrupt_14();
	fn vec_interrupt_15();
	fn vec_interrupt_16();
	fn vec_interrupt_17();
	fn vec_interrupt_18();
	fn vec_interrupt_19();
	fn vec_interrupt_20();
	fn vec_interrupt_21();
	fn vec_interrupt_22();
	fn vec_interrupt_23();
	fn vec_interrupt_24();
	fn vec_interrupt_25();
	fn vec_interrupt_26();
	fn vec_interrupt_27();
	fn vec_interrupt_28();
	fn vec_interrupt_29();
	fn vec_interrupt_30();
	fn vec_interrupt_31();
	fn vec_interrupt_32();
	fn vec_interrupt_33();
	fn vec_interrupt_34();
	fn vec_interrupt_35();
	fn vec_interrupt_36();
	fn vec_interrupt_37();
	fn vec_interrupt_38();
	fn vec_interrupt_39();
	fn vec_interrupt_40();
	fn vec_interrupt_41();
	fn vec_interrupt_42();
	fn vec_interrupt_43();
	fn vec_interrupt_44();
	fn vec_interrupt_45();
	fn vec_interrupt_46();
	fn vec_interrupt_47();
	fn vec_interrupt_48();
	fn vec_interrupt_49();
	fn vec_interrupt_50();
	fn vec_interrupt_51();
	fn vec_interrupt_52();
	fn vec_interrupt_53();
	fn vec_interrupt_54();
	fn vec_interrupt_55();
	fn vec_interrupt_56();
	fn vec_interrupt_57();
	fn vec_interrupt_58();
	fn vec_interrupt_59();
	fn vec_interrupt_60();
	fn vec_interrupt_61();
	fn vec_interrupt_62();
	fn vec_interrupt_63();
	fn vec_interrupt_64();
	fn vec_interrupt_65();
	fn vec_interrupt_66();
	fn vec_interrupt_67();
	fn vec_interrupt_68();
	fn vec_interrupt_69();
	fn vec_interrupt_70();
	fn vec_interrupt_71();
	fn vec_interrupt_72();
	fn vec_interrupt_73();
	fn vec_interrupt_74();
	fn vec_interrupt_75();
	fn vec_interrupt_76();
	fn vec_interrupt_77();
	fn vec_interrupt_78();
	fn vec_interrupt_79();
	fn vec_interrupt_80();
	fn vec_interrupt_81();
	fn vec_interrupt_82();
	fn vec_interrupt_83();
	fn vec_interrupt_84();
	fn vec_interrupt_85();
	fn vec_interrupt_86();
	fn vec_interrupt_87();
	fn vec_interrupt_88();
	fn vec_interrupt_89();
	fn vec_interrupt_90();
	fn vec_interrupt_91();
	fn vec_interrupt_92();
	fn vec_interrupt_93();
	fn vec_interrupt_94();
	fn vec_interrupt_95();
	fn vec_interrupt_96();
	fn vec_interrupt_97();
	fn vec_interrupt_98();
	fn vec_interrupt_99();
	fn vec_interrupt_100();
	fn vec_interrupt_101();
	fn vec_interrupt_102();
	fn vec_interrupt_103();
	fn vec_interrupt_104();
	fn vec_interrupt_105();
	fn vec_interrupt_106();
	fn vec_interrupt_107();
	fn vec_interrupt_108();
	fn vec_interrupt_109();
	fn vec_interrupt_110();
	fn vec_interrupt_111();
	fn vec_interrupt_112();
	fn vec_interrupt_113();
	fn vec_interrupt_114();
	fn vec_interrupt_115();
	fn vec_interrupt_116();
	fn vec_interrupt_117();
	fn vec_interrupt_118();
	fn vec_interrupt_119();
	fn vec_interrupt_120();
	fn vec_interrupt_121();
	fn vec_interrupt_122();
	fn vec_interrupt_123();
	fn vec_interrupt_124();
	fn vec_interrupt_125();
	fn vec_interrupt_126();
	fn vec_interrupt_127();
	fn vec_interrupt_128();
	fn vec_interrupt_129();
	fn vec_interrupt_130();
	fn vec_interrupt_131();
	fn vec_interrupt_132();
	fn vec_interrupt_133();
	fn vec_interrupt_134();
	fn vec_interrupt_135();
	fn vec_interrupt_136();
	fn vec_interrupt_137();
	fn vec_interrupt_138();
	fn vec_interrupt_139();
	fn vec_interrupt_140();
	fn vec_interrupt_141();
	fn vec_interrupt_142();
	fn vec_interrupt_143();
	fn vec_interrupt_144();
	fn vec_interrupt_145();
	fn vec_interrupt_146();
	fn vec_interrupt_147();
	fn vec_interrupt_148();
	fn vec_interrupt_149();
	fn vec_interrupt_150();
	fn vec_interrupt_151();
	fn vec_interrupt_152();
	fn vec_interrupt_153();
	fn vec_interrupt_154();
	fn vec_interrupt_155();
	fn vec_interrupt_156();
	fn vec_interrupt_157();
	fn vec_interrupt_158();
	fn vec_interrupt_159();
	fn vec_interrupt_160();
	fn vec_interrupt_161();
	fn vec_interrupt_162();
	fn vec_interrupt_163();
	fn vec_interrupt_164();
	fn vec_interrupt_165();
	fn vec_interrupt_166();
	fn vec_interrupt_167();
	fn vec_interrupt_168();
	fn vec_interrupt_169();
	fn vec_interrupt_170();
	fn vec_interrupt_171();
	fn vec_interrupt_172();
	fn vec_interrupt_173();
	fn vec_interrupt_174();
	fn vec_interrupt_175();
	fn vec_interrupt_176();
	fn vec_interrupt_177();
	fn vec_interrupt_178();
	fn vec_interrupt_179();
	fn vec_interrupt_180();
	fn vec_interrupt_181();
	fn vec_interrupt_182();
	fn vec_interrupt_183();
	fn vec_interrupt_184();
	fn vec_interrupt_185();
	fn vec_interrupt_186();
	fn vec_interrupt_187();
	fn vec_interrupt_188();
	fn vec_interrupt_189();
	fn vec_interrupt_190();
	fn vec_interrupt_191();
	fn vec_interrupt_192();
	fn vec_interrupt_193();
	fn vec_interrupt_194();
	fn vec_interrupt_195();
	fn vec_interrupt_196();
	fn vec_interrupt_197();
	fn vec_interrupt_198();
	fn vec_interrupt_199();
	fn vec_interrupt_200();
	fn vec_interrupt_201();
	fn vec_interrupt_202();
	fn vec_interrupt_203();
	fn vec_interrupt_204();
	fn vec_interrupt_205();
	fn vec_interrupt_206();
	fn vec_interrupt_207();
	fn vec_interrupt_208();
	fn vec_interrupt_209();
	fn vec_interrupt_210();
	fn vec_interrupt_211();
	fn vec_interrupt_212();
	fn vec_interrupt_213();
	fn vec_interrupt_214();
	fn vec_interrupt_215();
	fn vec_interrupt_216();
	fn vec_interrupt_217();
	fn vec_interrupt_218();
	fn vec_interrupt_219();
	fn vec_interrupt_220();
	fn vec_interrupt_221();
	fn vec_interrupt_222();
	fn vec_interrupt_223();
	fn vec_interrupt_224();
	fn vec_interrupt_225();
	fn vec_interrupt_226();
	fn vec_interrupt_227();
	fn vec_interrupt_228();
	fn vec_interrupt_229();
	fn vec_interrupt_230();
	fn vec_interrupt_231();
	fn vec_interrupt_232();
	fn vec_interrupt_233();
	fn vec_interrupt_234();
	fn vec_interrupt_235();
	fn vec_interrupt_236();
	fn vec_interrupt_237();
	fn vec_interrupt_238();
	fn vec_interrupt_239();
	fn vec_interrupt_240();
	fn vec_interrupt_241();
	fn vec_interrupt_242();
	fn vec_interrupt_243();
	fn vec_interrupt_244();
	fn vec_interrupt_245();
	fn vec_interrupt_246();
	fn vec_interrupt_247();
	fn vec_interrupt_248();
	fn vec_interrupt_249();
	fn vec_interrupt_250();
	fn vec_interrupt_251();
	fn vec_interrupt_252();
	fn vec_interrupt_253();
	fn vec_interrupt_254();
	fn vec_interrupt_255();
}

global_asm!(r#"
.intel_syntax

.macro XMMPUSH reg
    sub    rsp, 16
    movdqu [rsp], \reg
.endm

.macro XMMPOP reg
    movdqu \reg, [rsp]
    add    rsp, 16
.endm

.extern interrupt_handler

enter_rust:
	push rax
	push rbx
	push rcx
	push rdx
	push rsi
	push rdi
	push rbp
	push r8
	push r9
	push r10
	push r11
	push r12
	push r13
	push r14
	push r15

    XMMPUSH xmm0
    XMMPUSH xmm1
    XMMPUSH xmm2
    XMMPUSH xmm3
    XMMPUSH xmm4
    XMMPUSH xmm5
    XMMPUSH xmm6
    XMMPUSH xmm7
    XMMPUSH xmm8
    XMMPUSH xmm9
    XMMPUSH xmm10
    XMMPUSH xmm11
    XMMPUSH xmm12
    XMMPUSH xmm13
    XMMPUSH xmm14
    XMMPUSH xmm15

	// Call the rust interrupt handler
    sub  rsp, 0x20
	call interrupt_handler
    add  rsp, 0x20

    XMMPOP xmm15
    XMMPOP xmm14
    XMMPOP xmm13
    XMMPOP xmm12
    XMMPOP xmm11
    XMMPOP xmm10
    XMMPOP xmm9
    XMMPOP xmm8
    XMMPOP xmm7
    XMMPOP xmm6
    XMMPOP xmm5
    XMMPOP xmm4
    XMMPOP xmm3
    XMMPOP xmm2
    XMMPOP xmm1
    XMMPOP xmm0

	pop r15
	pop r14
	pop r13
	pop r12
	pop r11
	pop r10
	pop r9
	pop r8
	pop rbp
	pop rdi
	pop rsi
	pop rdx
	pop rcx
	pop rbx
	pop rax
	ret

.macro define_int_handler int_id, has_error_code
.global vec_interrupt_\int_id
vec_interrupt_\int_id:
	push rcx
	push rdx
	push r8

.if \has_error_code
	mov  rcx, \int_id
	lea  rdx, [rsp+0x20]
	mov  r8,  [rsp+0x18]
	
	// 16-byte align the stack
	sub rsp, 8
.else
	mov rcx, \int_id
	lea rdx, [rsp+0x18]
	mov r8,  0
.endif

	call enter_rust
	
.if \has_error_code
	// Remove alignment from before
	add rsp, 8
.endif

	pop r8
	pop rdx
	pop rcx

.if \has_error_code
	// 'pop' off the error code
	add rsp, 8
.endif

	iretq
.endm

define_int_handler 0, 0
define_int_handler 1, 0
define_int_handler 2, 0
define_int_handler 3, 0
define_int_handler 4, 0
define_int_handler 5, 0
define_int_handler 6, 0
define_int_handler 7, 0
define_int_handler 8, 1
define_int_handler 9, 0
define_int_handler 10, 1
define_int_handler 11, 1
define_int_handler 12, 1
define_int_handler 13, 1
define_int_handler 14, 1
define_int_handler 15, 0
define_int_handler 16, 0
define_int_handler 17, 1
define_int_handler 18, 0
define_int_handler 19, 0
define_int_handler 20, 0
define_int_handler 21, 0
define_int_handler 22, 0
define_int_handler 23, 0
define_int_handler 24, 0
define_int_handler 25, 0
define_int_handler 26, 0
define_int_handler 27, 0
define_int_handler 28, 0
define_int_handler 29, 0
define_int_handler 30, 0
define_int_handler 31, 0
define_int_handler 32, 0
define_int_handler 33, 0
define_int_handler 34, 0
define_int_handler 35, 0
define_int_handler 36, 0
define_int_handler 37, 0
define_int_handler 38, 0
define_int_handler 39, 0
define_int_handler 40, 0
define_int_handler 41, 0
define_int_handler 42, 0
define_int_handler 43, 0
define_int_handler 44, 0
define_int_handler 45, 0
define_int_handler 46, 0
define_int_handler 47, 0
define_int_handler 48, 0
define_int_handler 49, 0
define_int_handler 50, 0
define_int_handler 51, 0
define_int_handler 52, 0
define_int_handler 53, 0
define_int_handler 54, 0
define_int_handler 55, 0
define_int_handler 56, 0
define_int_handler 57, 0
define_int_handler 58, 0
define_int_handler 59, 0
define_int_handler 60, 0
define_int_handler 61, 0
define_int_handler 62, 0
define_int_handler 63, 0
define_int_handler 64, 0
define_int_handler 65, 0
define_int_handler 66, 0
define_int_handler 67, 0
define_int_handler 68, 0
define_int_handler 69, 0
define_int_handler 70, 0
define_int_handler 71, 0
define_int_handler 72, 0
define_int_handler 73, 0
define_int_handler 74, 0
define_int_handler 75, 0
define_int_handler 76, 0
define_int_handler 77, 0
define_int_handler 78, 0
define_int_handler 79, 0
define_int_handler 80, 0
define_int_handler 81, 0
define_int_handler 82, 0
define_int_handler 83, 0
define_int_handler 84, 0
define_int_handler 85, 0
define_int_handler 86, 0
define_int_handler 87, 0
define_int_handler 88, 0
define_int_handler 89, 0
define_int_handler 90, 0
define_int_handler 91, 0
define_int_handler 92, 0
define_int_handler 93, 0
define_int_handler 94, 0
define_int_handler 95, 0
define_int_handler 96, 0
define_int_handler 97, 0
define_int_handler 98, 0
define_int_handler 99, 0
define_int_handler 100, 0
define_int_handler 101, 0
define_int_handler 102, 0
define_int_handler 103, 0
define_int_handler 104, 0
define_int_handler 105, 0
define_int_handler 106, 0
define_int_handler 107, 0
define_int_handler 108, 0
define_int_handler 109, 0
define_int_handler 110, 0
define_int_handler 111, 0
define_int_handler 112, 0
define_int_handler 113, 0
define_int_handler 114, 0
define_int_handler 115, 0
define_int_handler 116, 0
define_int_handler 117, 0
define_int_handler 118, 0
define_int_handler 119, 0
define_int_handler 120, 0
define_int_handler 121, 0
define_int_handler 122, 0
define_int_handler 123, 0
define_int_handler 124, 0
define_int_handler 125, 0
define_int_handler 126, 0
define_int_handler 127, 0
define_int_handler 128, 0
define_int_handler 129, 0
define_int_handler 130, 0
define_int_handler 131, 0
define_int_handler 132, 0
define_int_handler 133, 0
define_int_handler 134, 0
define_int_handler 135, 0
define_int_handler 136, 0
define_int_handler 137, 0
define_int_handler 138, 0
define_int_handler 139, 0
define_int_handler 140, 0
define_int_handler 141, 0
define_int_handler 142, 0
define_int_handler 143, 0
define_int_handler 144, 0
define_int_handler 145, 0
define_int_handler 146, 0
define_int_handler 147, 0
define_int_handler 148, 0
define_int_handler 149, 0
define_int_handler 150, 0
define_int_handler 151, 0
define_int_handler 152, 0
define_int_handler 153, 0
define_int_handler 154, 0
define_int_handler 155, 0
define_int_handler 156, 0
define_int_handler 157, 0
define_int_handler 158, 0
define_int_handler 159, 0
define_int_handler 160, 0
define_int_handler 161, 0
define_int_handler 162, 0
define_int_handler 163, 0
define_int_handler 164, 0
define_int_handler 165, 0
define_int_handler 166, 0
define_int_handler 167, 0
define_int_handler 168, 0
define_int_handler 169, 0
define_int_handler 170, 0
define_int_handler 171, 0
define_int_handler 172, 0
define_int_handler 173, 0
define_int_handler 174, 0
define_int_handler 175, 0
define_int_handler 176, 0
define_int_handler 177, 0
define_int_handler 178, 0
define_int_handler 179, 0
define_int_handler 180, 0
define_int_handler 181, 0
define_int_handler 182, 0
define_int_handler 183, 0
define_int_handler 184, 0
define_int_handler 185, 0
define_int_handler 186, 0
define_int_handler 187, 0
define_int_handler 188, 0
define_int_handler 189, 0
define_int_handler 190, 0
define_int_handler 191, 0
define_int_handler 192, 0
define_int_handler 193, 0
define_int_handler 194, 0
define_int_handler 195, 0
define_int_handler 196, 0
define_int_handler 197, 0
define_int_handler 198, 0
define_int_handler 199, 0
define_int_handler 200, 0
define_int_handler 201, 0
define_int_handler 202, 0
define_int_handler 203, 0
define_int_handler 204, 0
define_int_handler 205, 0
define_int_handler 206, 0
define_int_handler 207, 0
define_int_handler 208, 0
define_int_handler 209, 0
define_int_handler 210, 0
define_int_handler 211, 0
define_int_handler 212, 0
define_int_handler 213, 0
define_int_handler 214, 0
define_int_handler 215, 0
define_int_handler 216, 0
define_int_handler 217, 0
define_int_handler 218, 0
define_int_handler 219, 0
define_int_handler 220, 0
define_int_handler 221, 0
define_int_handler 222, 0
define_int_handler 223, 0
define_int_handler 224, 0
define_int_handler 225, 0
define_int_handler 226, 0
define_int_handler 227, 0
define_int_handler 228, 0
define_int_handler 229, 0
define_int_handler 230, 0
define_int_handler 231, 0
define_int_handler 232, 0
define_int_handler 233, 0
define_int_handler 234, 0
define_int_handler 235, 0
define_int_handler 236, 0
define_int_handler 237, 0
define_int_handler 238, 0
define_int_handler 239, 0
define_int_handler 240, 0
define_int_handler 241, 0
define_int_handler 242, 0
define_int_handler 243, 0
define_int_handler 244, 0
define_int_handler 245, 0
define_int_handler 246, 0
define_int_handler 247, 0
define_int_handler 248, 0
define_int_handler 249, 0
define_int_handler 250, 0
define_int_handler 251, 0
define_int_handler 252, 0
define_int_handler 253, 0
define_int_handler 254, 0
define_int_handler 255, 0

.att_syntax

"#);

