#![no_std]
#![no_main]
#![feature(core_intrinsics)]
#![feature(alloc_error_handler)]

extern crate alloc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::ptr::write_volatile;
use core::time::Duration;

// ==================== ALOCADOR BUMP ====================
const HEAP_SIZE: usize = 1024 * 1024;
static mut HEAP: [u8; HEAP_SIZE] = [0u8; HEAP_SIZE];
static mut CURRENT: usize = 0;

pub fn init_heap() {
    unsafe { CURRENT = 0; }
}

#[global_allocator]
static HEAP_ALLOCATOR: BumpAllocator = BumpAllocator;

struct BumpAllocator;

unsafe impl core::alloc::GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let start = HEAP.as_ptr() as usize + CURRENT;
        let aligned_start = (start + layout.align() - 1) & !(layout.align() - 1);
        let new_current = aligned_start + layout.size();
        if new_current > HEAP.as_ptr() as usize + HEAP_SIZE {
            return core::ptr::null_mut();
        }
        CURRENT = new_current;
        aligned_start as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: core::alloc::Layout) {
        // Bump allocator não liberta memória individualmente
    }
}

#[alloc_error_handler]
fn alloc_error(layout: core::alloc::Layout) -> ! {
    VGA::write_str("\n[ALOC] Falha ao alocar memory (size: {}, align: {})\n", layout.size(), layout.align());
    loop { x86_64::instructions::hlt(); }
}

// ==================== CONSOLE VGA/VT100 ====================
const VGA_WIDTH: usize = 80;
const VGA_HEIGHT: usize = 25;
const VGA_BUFFER: *mut u16 = 0xB8000 as *mut u16;

impl VGA {
    pub fn clear() {
        let buf = unsafe { &mut *VGA_BUFFER };
        for i in 0..(VGA_WIDTH * VGA_HEIGHT) {
            buf[i] = 0x0720; // Fundo preto, branco, espaço
        }
    }

    pub fn write_str(&'static self, fmt: core::fmt::Arguments) {
        let s = format!("{}", fmt);
        let mut col: usize = 0;
        let mut row: usize = 0;
        
        for c in s.chars() {
            if c == '\n' { col = 0; row += 1; continue; }
            if c == '\r' { continue; }
            
            // Cursor virtual simples
            if row >= VGA_HEIGHT {
                Self::clear();
                row = VGA_HEIGHT - 1;
            }
            
            let addr = unsafe { VGA_BUFFER.add(row * VGA_WIDTH + col) };
            write_volatile(addr, (0x07u16 << 8) | (c as u16));
            col += 1;
        }
    }

    pub fn set_color(&'static self, fg: u8, bg: u8) {
        let _ = (fg, bg); // Implementação futura para atributos de sessão
    }
}

// ==================== TECLADO (Polling) ====================
const KB_DATA_PORT: u16 = 0x60;
const KB_STATUS_PORT: u16 = 0x64;

#[inline]
fn inb(port: u16) -> u8 {
    unsafe { core::arch::x86_64::_inportb(port) }
}

pub struct Kbd {
    buffer: Vec<u8>,
}

impl Kbd {
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    pub fn poll(&mut self) -> Option<String> {
        if inb(KB_STATUS_PORT) & 1 == 0 { return None; }
        
        let sc = inb(KB_DATA_PORT);
        let ascii = scancode_to_ascii(sc);
        
        match ascii {
            Some('\n') | Some('\r') => {
                // Nova linha processada
                Some(self.buffer.drain(..).collect())
            }
            Some('\x08') | Some('\x7F') => {
                self.buffer.pop();
                VGA::write_str(format_args!("\x08 \x08"));
                None
            }
            Some(c) if c.is_ascii_graphic() || c.is_ascii_alphanumeric() => {
                self.buffer.push(sc);
                VGA::write_str(format_args!("{}", c));
                None
            }
            _ => None,
        }
    }
}

fn scancode_to_ascii(code: u8) -> Option<char> {
    match code {
        0x1C => Some('\n'),
        0x0E => Some('\x08'),
        0x39..=0x52 => Some((code - 0x30) as char), // 0-9
        0x1E..=0x30 => Some(((code - 0x1E) + b'a') as char), // a-z
        0x3B => Some('q'), // Ajuste simples para demo
        0x2C => Some('w'),
        0x32 => Some('e'),
        0x21 => Some('r'),
        0x23 => Some('t'),
        0x24 => Some('y'),
        0x2D => Some('u'),
        0x2E => Some('i'),
        0x2F => Some('o'),
        0x33 => Some('p'),
        0x1A => Some('a'),
        0x1B => Some('s'),
        0x1C => Some('d'), // Overlap intentional simplificado
        _ => None,
    }
}

// ==================== VFS (Arquivos de Fábrica) ====================
pub struct VFS;

impl VFS {
    pub fn init_factory_files() {
        // Estrutura pré-carregada inspirada no Arch Linux
        VGA::write_str(format_args!("[VFS] Carregando sistema de arquivos virtual...\n"));
    }

    pub fn ls(path: &str) -> &'static str {
        match path {
            "/" => "etc  home  var  tmp",
            "/home" => "user",
            "/home/user" => "documents  .bashrc  readme.md",
            "/etc" => "arch-release  pacman.conf",
            "/var" => "log  pacman.log",
            _ => "",
        }
    }

    pub fn cat(path: &str) -> &'static str {
        match path {
            "/etc/arch-release" => "VIRTUS OS 2026.09\nKernel: Virtus/1.0.0\nArchitecture: x86_64",
            "/home/user/.bashrc" => "# ~/.bashrc\nexport PATH=/usr/bin:/bin:/usr/sbin:/sbin\nalias update='virtus-pkg -Syu'",
            "/home/user/readme.md" => "# VIRTUS OS\nMinimalist. Rolling. DIY.\nRun `help` in the terminal.",
            "/var/log/pacman.log" => "[2026-09-27 18:30] Starting full system upgrade\n[2026-09-27 18:31] Resolving dependencies...\n[done]",
            _ => "cat: arquivo não encontrado",
        }
    }
}

// ==================== SHELL & PACMAN ====================
pub struct Shell {
    prompt: &'static str,
    cwd: String,
}

impl Shell {
    pub fn run() -> ! {
        let mut shell = Self { 
            prompt: "virtus@host ~ $ ", 
            cwd: "/home/user".into() 
        };
        
        VGA::clear();
        VGA::write_str(format_args!("\x1B[32m
╔══════════════════════════════════════╗
║          VIRTUS OS 2026.09           ║
║   Minimalist. Rolling. DIY.          ║