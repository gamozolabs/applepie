all:
	cd kernel && cargo build --release
	python flatten_pe.py kernel\target\x86_64-pc-windows-msvc\release\kernel.exe kernel.flat
	nasm -f bin -o unnamed_os.boot.vfd bootloader.asm

debug:
	cd kernel && cargo build
	python flatten_pe.py kernel\target\x86_64-pc-windows-msvc\debug\kernel.exe kernel.flat
	nasm -f bin -o unnamed_os.boot.vfd bootloader.asm

run: all
	..\..\bochs_build\bochs.exe -q -f bochsrc.bxrc
	
clean:
	-cd kernel && cargo clean
	-del unnamed_os.boot.vfd kernel.flat 2>NUL

