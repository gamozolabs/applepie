set BXSHARE=..\bochs\bios
nasm -f bin test.asm -o test.bin
..\bochs_build\bochs.exe -q -unlock -f bochsrc.bxrc
