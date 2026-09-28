ENTRY(_start)

SECTIONS {
    . = 1M;
    
    .text : {
        *(.text.entry)
        *(.text.*)
    }

    .data : {
        *(.data.*)
    }

    .bss : {
        *(.bss.*)
        . = ALIGN(8);
        __stack_top = .;
    }

    /DISCARD/ : {
        *(.note*)
        *(.eh_frame*)
    }
}