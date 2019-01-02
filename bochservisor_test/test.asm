[bits 16]
[org 0x7c00]

loop:
    mov dx, 0x1337
    out dx, al
    
    mov ax, 0
    mov es, ax
    mov di, ax

    mov di, 0xb800
    mov es, di
    xor di, di
    mov ax, 0x0530
    mov cx, 80
    rep stosw

.halt:
    hlt
    jmp short .halt

times 510-($-$$) db 0
dw 0xaa55
