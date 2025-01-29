```c
void check_callee(void) {
    // should have a push and pop of all of these:
    // r10=sl
    // r11=fp
    // r12=ip
    // r13=sp
    // r14=ir
    // r15=pc
    asm volatile ( "nop" : : : "r4", "r5", "r6", "r7", "r8", "r9", "r10", "r11");
}
```
