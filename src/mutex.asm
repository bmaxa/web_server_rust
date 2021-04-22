format elf64

FUTEX_WAIT   equ           0
FUTEX_WAKE   equ           1
FUTEX_PRIVATE_FLAG equ     128

public futex_acquire
public futex_release

futex_acquire: 
	push rbx
	push r15
;	push r10
	mov r15,rdi
.L0:
        mov ebx,1
        xor eax,eax
        lock cmpxchg [r15],ebx
        test eax,eax
        jz .L1
        mov eax, 202
        mov rdi, r15
        mov rsi, FUTEX_WAIT or FUTEX_PRIVATE_FLAG
        mov edx, 1
        xor r10,r10
        syscall
        jmp .L0
.L1:;	pop r10
	pop r15
	pop rbx
	ret

futex_release:
        lock and dword[rdi],0
        mov eax,202
;        mov rdi, sema
        mov rsi, FUTEX_WAKE or FUTEX_PRIVATE_FLAG
        mov edx,1
        syscall  
	ret
