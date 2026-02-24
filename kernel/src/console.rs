use core::fmt::{self, Write};

use spin::Mutex;

const COM1_PORT: u16 = 0x3f8;
const LSR_THR_EMPTY: u8 = 1 << 5;

pub static SERIAL1: Mutex<SerialPort> = Mutex::new(SerialPort::new(COM1_PORT));

pub fn init() {
    SERIAL1.lock().init();
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments<'_>) {
    let _ = SERIAL1.lock().write_fmt(args);
}

pub struct SerialPort {
    base_port: u16,
}

impl SerialPort {
    pub const fn new(base_port: u16) -> Self {
        Self { base_port }
    }

    pub fn init(&mut self) {
        // Disable interrupts.
        self.write_reg(1, 0x00);
        // Enable DLAB.
        self.write_reg(3, 0x80);
        // Set divisor to 3 => 38400 baud for a 115200 Hz clock.
        self.write_reg(0, 0x03);
        self.write_reg(1, 0x00);
        // 8 bits, no parity, one stop bit.
        self.write_reg(3, 0x03);
        // Enable FIFO, clear queues, 14-byte threshold.
        self.write_reg(2, 0xC7);
        // IRQs disabled, RTS/DSR set.
        self.write_reg(4, 0x03);
    }

    fn write_reg(&self, offset: u16, value: u8) {
        outb(self.base_port + offset, value);
    }

    fn read_reg(&self, offset: u16) -> u8 {
        inb(self.base_port + offset)
    }

    fn write_byte(&self, byte: u8) {
        while self.read_reg(5) & LSR_THR_EMPTY == 0 {}
        self.write_reg(0, byte);
    }
}

impl Write for SerialPort {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            if byte == b'\n' {
                self.write_byte(b'\r');
            }
            self.write_byte(byte);
        }
        Ok(())
    }
}

#[inline]
fn outb(port: u16, value: u8) {
    unsafe {
        core::arch::asm!(
            "out dx, al",
            in("dx") port,
            in("al") value,
            options(nomem, nostack, preserves_flags),
        );
    }
}

#[inline]
fn inb(port: u16) -> u8 {
    let value: u8;
    unsafe {
        core::arch::asm!(
            "in al, dx",
            in("dx") port,
            out("al") value,
            options(nomem, nostack, preserves_flags),
        );
    }
    value
}
