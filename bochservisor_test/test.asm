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

    mov byte [0x8000], 0x12
    mov byte [0x8001], 0x34
    mov byte [0x8002], 0x56
    mov byte [0x8003], 0x78
    mov byte [0x8004], 0x9a
    mov byte [0x8005], 0xbc
    mov byte [0x8006], 0xde
    mov byte [0x8007], 0xf0
    mov byte [0x8008], 0x99

.halt:
    hlt
    jmp short .halt

times 510-($-$$) db 0
dw 0xaa55
