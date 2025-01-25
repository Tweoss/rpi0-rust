SECTIONS
{
    __code_start__ = .;
    .text 0x8000 :  { *(.text.start) *(.text*) }
    __code_end__ = .;
    __data_start__ = .;
    .rodata :       { *(.rodata*) }
    .data :         { *(.data*) }
    __data_end__ = .;
    __bss_start__ = .;
    .bss :          { *(.bss*)  *(COMMON) }
    __bss_end__ = ALIGN(8);
}

/* Force link of _start and verify correct position */
ENTRY(_start)
ASSERT(_start == ADDR(.text), "_start symbol must be placed first in text section")
