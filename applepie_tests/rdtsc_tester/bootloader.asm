[org  0x7c00]
[bits 16]

%define BYTES_PER_SECTOR   512
%define SECTORS_PER_TRACK   18
%define HEADS_PER_CYLINDER   2
%define CYLINDERS           80
%define CYL_TO_READ          8

%define READ_TARGET 0x7e00

entry:
	; Disable interrupts and clear direction flag
	cli
	cld

	; Save off the boot drive letter
	mov byte [drive_letter], dl

	; Clear DS, ES, and CS and set up a stack
	mov sp, 0x7c00
	xor ax, ax
	mov es, ax
	mov ds, ax
	mov fs, ax
	mov gs, ax
	mov ss, ax
	jmp 0x0000:.clear_cs

.clear_cs:
	; Go into VGA mode 3 (80x25 16-colour mode)
	mov ax, 0x0003
	int 0x10

	call print_status ; A

	; Check for SSE
	mov eax, 1
	cpuid
	test edx, 1 << 25
	jz   rm_halt

	call print_status ; B

	; Drive reset
	xor ah, ah
	mov dl, byte [drive_letter]
	int 0x13
	jc  rm_halt

	call print_status ; C

	; Get drive parameters
	mov ah, 0x08
	mov dl, byte [drive_letter]
	int 0x13
	jc  rm_halt

	call print_status ; D

	; Make sure sectors per track == 18 and cylinders == 80
	cmp cx, 0x4f12
	jne rm_halt

	call print_status ; E

.lewp:
	mov ah, 0x02 ; Read disk sectors
	mov al, 1    ; Read 1 sector

	mov ch, byte [cyl]
	mov cl, byte [sect]

	mov dh, byte [head]
	mov dl, byte [drive_letter]

	mov  ebx, dword [buffer] ; Address to read to
	call linear_to_segoff
	add  dword [buffer], BYTES_PER_SECTOR ; Update address

	; Read the data and check for error
	int 0x13
	cli
	jc rm_halt

	; Increment sector, make sure it's in bounds
	add word [sect], 1
	cmp word [sect], SECTORS_PER_TRACK
	jbe .sectors_good

	; Reset sector and roll over count to head
	mov word [sect], 1
	inc word [head]
	cmp word [head], HEADS_PER_CYLINDER
	jb  .heads_good

	; Reset head and increment cylinder
	mov word [head], 0
	inc word [cyl]

.heads_good:
.sectors_good:
	cmp word [cyl], CYL_TO_READ
	jae .end_loop
	jmp .lewp

.end_loop:
	call print_status ; F

	; Initialize the IVT to catch all exceptions at this point as we're about
	; to try going to long mode
	mov bx, 0
	mov cx, 256
.ivt_loop:
	mov word [bx + 0], rm_halt ; Offset
	mov word [bx + 2], 0       ; Segment
	add bx, 4
	dec cx
	jnz short .ivt_loop

	; Set the A20 line
	in    al, 0x92
	or    al, 2
	out 0x92, al

	call print_status ; G

	; Set this as the active page table
	mov ebx, pml4
	mov cr3, ebx

	call print_status ; H

	; Set NXE (NX enable), LME (long mode enable), and SCE (syscall enable).
	mov edx, 0
	mov eax, 0x00000901
	mov ecx, 0xc0000080
	wrmsr

	call print_status ; I

	; Set OSXMMEXCPT, OSFXSR, PAE, and DE
	mov eax, 0x628
	mov cr4, eax

	call print_status ; J

	; Set paging enable, write protect, extension type, monitor coprocessor,
	; and protection enable
	mov eax, 0x80010013
	mov cr0, eax

	; Load the 64-bit long mode GDT
	lgdt [lmgdt]

	; Long jump to enable long mode!
	jmp 0x0008:bsp_lm_entry

print_status:
	pusha
	push es
	push ds

	; Scroll up screen
	mov di, 0xb800
	mov es, di
	mov ds, di
	xor di, di
	mov si, 80 * 2
	mov cx, 80 * 24 * 2
	rep movsb

	mov di, 0xb800
	mov es, di
	mov di, 80 * 24 * 2
	mov ah, 0x0f
	mov al, byte [fs:status]
	stosw

	inc byte [fs:status]

	pop ds
	pop es
	popa
	ret

rm_halt:
	cli
	hlt
	jmp short rm_halt

; ebx   -> Linear address
; es:bx <- Segoff
linear_to_segoff:
	push eax

	mov eax, ebx
	and eax, 0xf
	shr ebx, 4
	mov es, bx
	mov bx, ax

	pop eax
	ret

; ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

drive_letter: db 0
status:       db 'A'

cyl:    dd 0
head:   dd 0
sect:   dd 2
buffer: dd READ_TARGET

; ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

align 8
lmgdt_base:
	dq 0x0000000000000000 ; Null descriptor
	dq 0x00209a0000000000 ; 64-bit, present, code
	dq 0x0000920000000000 ; Present, data r/w

lmgdt:
	dw (lmgdt - lmgdt_base) - 1
	dq lmgdt_base

; ~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

[bits 64]

; This is the long-mode entry point for the BSP
bsp_lm_entry:
	; Disable interrupts and clear the direction flag
	cli
	cld

	; Reset segmentation
	mov ax, 0x10
	mov es, ax
	mov ds, ax
	mov fs, ax
	mov gs, ax
	mov ss, ax

	; Set up a stack
	mov rsp, 0x00080000

	; Get the address of the entry point
	mov rax, [rel kernel_end-8]

	; Call into the kernel!
	sub  rsp, 0x20
	call rax

	; Halt forever if we get here
.halt:
	cli
	hlt
	jmp short .halt
	
times 510-($-$$) db 0
dw 0xAA55

%define PT_BASE 0x8000

; Align so next address is 0x8000
times (0x400)-($-$$) db 0

pml4:
	dq (PT_BASE + 0x1000) | 7
	times 511 dq 0

pdp:
	dq (PT_BASE + 0x2000) | 7
	times 511 dq 0

pd:
	dq (PT_BASE + 0x3000) | 7
	times 511 dq 0

pt:
	dq 0x00000000 | 7
	dq 0x00001000 | 7
	dq 0x00002000 | 7
	dq 0x00003000 | 7
	dq 0x00004000 | 7
	dq 0x00005000 | 7
	dq 0x00006000 | 7
	dq 0x00007000 | 7
	dq 0x00008000 | 7
	dq 0x00009000 | 7
	dq 0x0000a000 | 7
	dq 0x0000b000 | 7
	dq 0x0000c000 | 7
	dq 0x0000d000 | 7
	dq 0x0000e000 | 7
	dq 0x0000f000 | 7
	dq 0x00010000 | 7
	dq 0x00011000 | 7
	dq 0x00012000 | 7
	dq 0x00013000 | 7
	dq 0x00014000 | 7
	dq 0x00015000 | 7
	dq 0x00016000 | 7
	dq 0x00017000 | 7
	dq 0x00018000 | 7
	dq 0x00019000 | 7
	dq 0x0001a000 | 7
	dq 0x0001b000 | 7
	dq 0x0001c000 | 7
	dq 0x0001d000 | 7
	dq 0x0001e000 | 7
	dq 0x0001f000 | 7
	dq 0x00020000 | 7
	dq 0x00021000 | 7
	dq 0x00022000 | 7
	dq 0x00023000 | 7
	dq 0x00024000 | 7
	dq 0x00025000 | 7
	dq 0x00026000 | 7
	dq 0x00027000 | 7
	dq 0x00028000 | 7
	dq 0x00029000 | 7
	dq 0x0002a000 | 7
	dq 0x0002b000 | 7
	dq 0x0002c000 | 7
	dq 0x0002d000 | 7
	dq 0x0002e000 | 7
	dq 0x0002f000 | 7
	dq 0x00030000 | 7
	dq 0x00031000 | 7
	dq 0x00032000 | 7
	dq 0x00033000 | 7
	dq 0x00034000 | 7
	dq 0x00035000 | 7
	dq 0x00036000 | 7
	dq 0x00037000 | 7
	dq 0x00038000 | 7
	dq 0x00039000 | 7
	dq 0x0003a000 | 7
	dq 0x0003b000 | 7
	dq 0x0003c000 | 7
	dq 0x0003d000 | 7
	dq 0x0003e000 | 7
	dq 0x0003f000 | 7
	dq 0x00040000 | 7
	dq 0x00041000 | 7
	dq 0x00042000 | 7
	dq 0x00043000 | 7
	dq 0x00044000 | 7
	dq 0x00045000 | 7
	dq 0x00046000 | 7
	dq 0x00047000 | 7
	dq 0x00048000 | 7
	dq 0x00049000 | 7
	dq 0x0004a000 | 7
	dq 0x0004b000 | 7
	dq 0x0004c000 | 7
	dq 0x0004d000 | 7
	dq 0x0004e000 | 7
	dq 0x0004f000 | 7
	dq 0x00050000 | 7
	dq 0x00051000 | 7
	dq 0x00052000 | 7
	dq 0x00053000 | 7
	dq 0x00054000 | 7
	dq 0x00055000 | 7
	dq 0x00056000 | 7
	dq 0x00057000 | 7
	dq 0x00058000 | 7
	dq 0x00059000 | 7
	dq 0x0005a000 | 7
	dq 0x0005b000 | 7
	dq 0x0005c000 | 7
	dq 0x0005d000 | 7
	dq 0x0005e000 | 7
	dq 0x0005f000 | 7
	dq 0x00060000 | 7
	dq 0x00061000 | 7
	dq 0x00062000 | 7
	dq 0x00063000 | 7
	dq 0x00064000 | 7
	dq 0x00065000 | 7
	dq 0x00066000 | 7
	dq 0x00067000 | 7
	dq 0x00068000 | 7
	dq 0x00069000 | 7
	dq 0x0006a000 | 7
	dq 0x0006b000 | 7
	dq 0x0006c000 | 7
	dq 0x0006d000 | 7
	dq 0x0006e000 | 7
	dq 0x0006f000 | 7
	dq 0x00070000 | 7
	dq 0x00071000 | 7
	dq 0x00072000 | 7
	dq 0x00073000 | 7
	dq 0x00074000 | 7
	dq 0x00075000 | 7
	dq 0x00076000 | 7
	dq 0x00077000 | 7
	dq 0x00078000 | 7
	dq 0x00079000 | 7
	dq 0x0007a000 | 7
	dq 0x0007b000 | 7
	dq 0x0007c000 | 7
	dq 0x0007d000 | 7
	dq 0x0007e000 | 7
	dq 0x0007f000 | 7
	dq 0x00080000 | 7
	dq 0x00081000 | 7
	dq 0x00082000 | 7
	dq 0x00083000 | 7
	dq 0x00084000 | 7
	dq 0x00085000 | 7
	dq 0x00086000 | 7
	dq 0x00087000 | 7
	dq 0x00088000 | 7
	dq 0x00089000 | 7
	dq 0x0008a000 | 7
	dq 0x0008b000 | 7
	dq 0x0008c000 | 7
	dq 0x0008d000 | 7
	dq 0x0008e000 | 7
	dq 0x0008f000 | 7
	dq 0x00090000 | 7
	dq 0x00091000 | 7
	dq 0x00092000 | 7
	dq 0x00093000 | 7
	dq 0x00094000 | 7
	dq 0x00095000 | 7
	dq 0x00096000 | 7
	dq 0x00097000 | 7
	dq 0x00098000 | 7
	dq 0x00099000 | 7
	dq 0x0009a000 | 7
	dq 0x0009b000 | 7
	dq 0x0009c000 | 7
	dq 0x0009d000 | 7
	dq 0x0009e000 | 7
	dq 0x0009f000 | 7
	dq 0x000a0000 | 7
	dq 0x000a1000 | 7
	dq 0x000a2000 | 7
	dq 0x000a3000 | 7
	dq 0x000a4000 | 7
	dq 0x000a5000 | 7
	dq 0x000a6000 | 7
	dq 0x000a7000 | 7
	dq 0x000a8000 | 7
	dq 0x000a9000 | 7
	dq 0x000aa000 | 7
	dq 0x000ab000 | 7
	dq 0x000ac000 | 7
	dq 0x000ad000 | 7
	dq 0x000ae000 | 7
	dq 0x000af000 | 7
	dq 0x000b0000 | 7
	dq 0x000b1000 | 7
	dq 0x000b2000 | 7
	dq 0x000b3000 | 7
	dq 0x000b4000 | 7
	dq 0x000b5000 | 7
	dq 0x000b6000 | 7
	dq 0x000b7000 | 7
	dq 0x000b8000 | 7
	dq 0x000b9000 | 7
	dq 0x000ba000 | 7
	dq 0x000bb000 | 7
	dq 0x000bc000 | 7
	dq 0x000bd000 | 7
	dq 0x000be000 | 7
	dq 0x000bf000 | 7
	dq 0x000c0000 | 7
	dq 0x000c1000 | 7
	dq 0x000c2000 | 7
	dq 0x000c3000 | 7
	dq 0x000c4000 | 7
	dq 0x000c5000 | 7
	dq 0x000c6000 | 7
	dq 0x000c7000 | 7
	dq 0x000c8000 | 7
	dq 0x000c9000 | 7
	dq 0x000ca000 | 7
	dq 0x000cb000 | 7
	dq 0x000cc000 | 7
	dq 0x000cd000 | 7
	dq 0x000ce000 | 7
	dq 0x000cf000 | 7
	dq 0x000d0000 | 7
	dq 0x000d1000 | 7
	dq 0x000d2000 | 7
	dq 0x000d3000 | 7
	dq 0x000d4000 | 7
	dq 0x000d5000 | 7
	dq 0x000d6000 | 7
	dq 0x000d7000 | 7
	dq 0x000d8000 | 7
	dq 0x000d9000 | 7
	dq 0x000da000 | 7
	dq 0x000db000 | 7
	dq 0x000dc000 | 7
	dq 0x000dd000 | 7
	dq 0x000de000 | 7
	dq 0x000df000 | 7
	dq 0x000e0000 | 7
	dq 0x000e1000 | 7
	dq 0x000e2000 | 7
	dq 0x000e3000 | 7
	dq 0x000e4000 | 7
	dq 0x000e5000 | 7
	dq 0x000e6000 | 7
	dq 0x000e7000 | 7
	dq 0x000e8000 | 7
	dq 0x000e9000 | 7
	dq 0x000ea000 | 7
	dq 0x000eb000 | 7
	dq 0x000ec000 | 7
	dq 0x000ed000 | 7
	dq 0x000ee000 | 7
	dq 0x000ef000 | 7
	dq 0x000f0000 | 7
	dq 0x000f1000 | 7
	dq 0x000f2000 | 7
	dq 0x000f3000 | 7
	dq 0x000f4000 | 7
	dq 0x000f5000 | 7
	dq 0x000f6000 | 7
	dq 0x000f7000 | 7
	dq 0x000f8000 | 7
	dq 0x000f9000 | 7
	dq 0x000fa000 | 7
	dq 0x000fb000 | 7
	dq 0x000fc000 | 7
	dq 0x000fd000 | 7
	dq 0x000fe000 | 7
	dq 0x000ff000 | 7
	dq 0x00100000 | 7
	dq 0x00101000 | 7
	dq 0x00102000 | 7
	dq 0x00103000 | 7
	dq 0x00104000 | 7
	dq 0x00105000 | 7
	dq 0x00106000 | 7
	dq 0x00107000 | 7
	dq 0x00108000 | 7
	dq 0x00109000 | 7
	dq 0x0010a000 | 7
	dq 0x0010b000 | 7
	dq 0x0010c000 | 7
	dq 0x0010d000 | 7
	dq 0x0010e000 | 7
	dq 0x0010f000 | 7
	dq 0x00110000 | 7
	dq 0x00111000 | 7
	dq 0x00112000 | 7
	dq 0x00113000 | 7
	dq 0x00114000 | 7
	dq 0x00115000 | 7
	dq 0x00116000 | 7
	dq 0x00117000 | 7
	dq 0x00118000 | 7
	dq 0x00119000 | 7
	dq 0x0011a000 | 7
	dq 0x0011b000 | 7
	dq 0x0011c000 | 7
	dq 0x0011d000 | 7
	dq 0x0011e000 | 7
	dq 0x0011f000 | 7
	dq 0x00120000 | 7
	dq 0x00121000 | 7
	dq 0x00122000 | 7
	dq 0x00123000 | 7
	dq 0x00124000 | 7
	dq 0x00125000 | 7
	dq 0x00126000 | 7
	dq 0x00127000 | 7
	dq 0x00128000 | 7
	dq 0x00129000 | 7
	dq 0x0012a000 | 7
	dq 0x0012b000 | 7
	dq 0x0012c000 | 7
	dq 0x0012d000 | 7
	dq 0x0012e000 | 7
	dq 0x0012f000 | 7
	dq 0x00130000 | 7
	dq 0x00131000 | 7
	dq 0x00132000 | 7
	dq 0x00133000 | 7
	dq 0x00134000 | 7
	dq 0x00135000 | 7
	dq 0x00136000 | 7
	dq 0x00137000 | 7
	dq 0x00138000 | 7
	dq 0x00139000 | 7
	dq 0x0013a000 | 7
	dq 0x0013b000 | 7
	dq 0x0013c000 | 7
	dq 0x0013d000 | 7
	dq 0x0013e000 | 7
	dq 0x0013f000 | 7
	dq 0x00140000 | 7
	dq 0x00141000 | 7
	dq 0x00142000 | 7
	dq 0x00143000 | 7
	dq 0x00144000 | 7
	dq 0x00145000 | 7
	dq 0x00146000 | 7
	dq 0x00147000 | 7
	dq 0x00148000 | 7
	dq 0x00149000 | 7
	dq 0x0014a000 | 7
	dq 0x0014b000 | 7
	dq 0x0014c000 | 7
	dq 0x0014d000 | 7
	dq 0x0014e000 | 7
	dq 0x0014f000 | 7
	dq 0x00150000 | 7
	dq 0x00151000 | 7
	dq 0x00152000 | 7
	dq 0x00153000 | 7
	dq 0x00154000 | 7
	dq 0x00155000 | 7
	dq 0x00156000 | 7
	dq 0x00157000 | 7
	dq 0x00158000 | 7
	dq 0x00159000 | 7
	dq 0x0015a000 | 7
	dq 0x0015b000 | 7
	dq 0x0015c000 | 7
	dq 0x0015d000 | 7
	dq 0x0015e000 | 7
	dq 0x0015f000 | 7
	dq 0x00160000 | 7
	dq 0x00161000 | 7
	dq 0x00162000 | 7
	dq 0x00163000 | 7
	dq 0x00164000 | 7
	dq 0x00165000 | 7
	dq 0x00166000 | 7
	dq 0x00167000 | 7
	dq 0x00168000 | 7
	dq 0x00169000 | 7
	dq 0x0016a000 | 7
	dq 0x0016b000 | 7
	dq 0x0016c000 | 7
	dq 0x0016d000 | 7
	dq 0x0016e000 | 7
	dq 0x0016f000 | 7
	dq 0x00170000 | 7
	dq 0x00171000 | 7
	dq 0x00172000 | 7
	dq 0x00173000 | 7
	dq 0x00174000 | 7
	dq 0x00175000 | 7
	dq 0x00176000 | 7
	dq 0x00177000 | 7
	dq 0x00178000 | 7
	dq 0x00179000 | 7
	dq 0x0017a000 | 7
	dq 0x0017b000 | 7
	dq 0x0017c000 | 7
	dq 0x0017d000 | 7
	dq 0x0017e000 | 7
	dq 0x0017f000 | 7
	dq 0x00180000 | 7
	dq 0x00181000 | 7
	dq 0x00182000 | 7
	dq 0x00183000 | 7
	dq 0x00184000 | 7
	dq 0x00185000 | 7
	dq 0x00186000 | 7
	dq 0x00187000 | 7
	dq 0x00188000 | 7
	dq 0x00189000 | 7
	dq 0x0018a000 | 7
	dq 0x0018b000 | 7
	dq 0x0018c000 | 7
	dq 0x0018d000 | 7
	dq 0x0018e000 | 7
	dq 0x0018f000 | 7
	dq 0x00190000 | 7
	dq 0x00191000 | 7
	dq 0x00192000 | 7
	dq 0x00193000 | 7
	dq 0x00194000 | 7
	dq 0x00195000 | 7
	dq 0x00196000 | 7
	dq 0x00197000 | 7
	dq 0x00198000 | 7
	dq 0x00199000 | 7
	dq 0x0019a000 | 7
	dq 0x0019b000 | 7
	dq 0x0019c000 | 7
	dq 0x0019d000 | 7
	dq 0x0019e000 | 7
	dq 0x0019f000 | 7
	dq 0x001a0000 | 7
	dq 0x001a1000 | 7
	dq 0x001a2000 | 7
	dq 0x001a3000 | 7
	dq 0x001a4000 | 7
	dq 0x001a5000 | 7
	dq 0x001a6000 | 7
	dq 0x001a7000 | 7
	dq 0x001a8000 | 7
	dq 0x001a9000 | 7
	dq 0x001aa000 | 7
	dq 0x001ab000 | 7
	dq 0x001ac000 | 7
	dq 0x001ad000 | 7
	dq 0x001ae000 | 7
	dq 0x001af000 | 7
	dq 0x001b0000 | 7
	dq 0x001b1000 | 7
	dq 0x001b2000 | 7
	dq 0x001b3000 | 7
	dq 0x001b4000 | 7
	dq 0x001b5000 | 7
	dq 0x001b6000 | 7
	dq 0x001b7000 | 7
	dq 0x001b8000 | 7
	dq 0x001b9000 | 7
	dq 0x001ba000 | 7
	dq 0x001bb000 | 7
	dq 0x001bc000 | 7
	dq 0x001bd000 | 7
	dq 0x001be000 | 7
	dq 0x001bf000 | 7
	dq 0x001c0000 | 7
	dq 0x001c1000 | 7
	dq 0x001c2000 | 7
	dq 0x001c3000 | 7
	dq 0x001c4000 | 7
	dq 0x001c5000 | 7
	dq 0x001c6000 | 7
	dq 0x001c7000 | 7
	dq 0x001c8000 | 7
	dq 0x001c9000 | 7
	dq 0x001ca000 | 7
	dq 0x001cb000 | 7
	dq 0x001cc000 | 7
	dq 0x001cd000 | 7
	dq 0x001ce000 | 7
	dq 0x001cf000 | 7
	dq 0x001d0000 | 7
	dq 0x001d1000 | 7
	dq 0x001d2000 | 7
	dq 0x001d3000 | 7
	dq 0x001d4000 | 7
	dq 0x001d5000 | 7
	dq 0x001d6000 | 7
	dq 0x001d7000 | 7
	dq 0x001d8000 | 7
	dq 0x001d9000 | 7
	dq 0x001da000 | 7
	dq 0x001db000 | 7
	dq 0x001dc000 | 7
	dq 0x001dd000 | 7
	dq 0x001de000 | 7
	dq 0x001df000 | 7
	dq 0x001e0000 | 7
	dq 0x001e1000 | 7
	dq 0x001e2000 | 7
	dq 0x001e3000 | 7
	dq 0x001e4000 | 7
	dq 0x001e5000 | 7
	dq 0x001e6000 | 7
	dq 0x001e7000 | 7
	dq 0x001e8000 | 7
	dq 0x001e9000 | 7
	dq 0x001ea000 | 7
	dq 0x001eb000 | 7
	dq 0x001ec000 | 7
	dq 0x001ed000 | 7
	dq 0x001ee000 | 7
	dq 0x001ef000 | 7
	dq 0x001f0000 | 7
	dq 0x001f1000 | 7
	dq 0x001f2000 | 7
	dq 0x001f3000 | 7
	dq 0x001f4000 | 7
	dq 0x001f5000 | 7
	dq 0x001f6000 | 7
	dq 0x001f7000 | 7
	dq 0x001f8000 | 7
	dq 0x001f9000 | 7
	dq 0x001fa000 | 7
	dq 0x001fb000 | 7
	dq 0x001fc000 | 7
	dq 0x001fd000 | 7
	dq 0x001fe000 | 7
	dq 0x001ff000 | 7

times (0x4400)-($-$$) db 0

kernel:
incbin "kernel.flat"
kernel_end:

times (CYL_TO_READ * HEADS_PER_CYLINDER * SECTORS_PER_TRACK * BYTES_PER_SECTOR)-($-$$) db 0
times (1474560)-($-$$) db 0

