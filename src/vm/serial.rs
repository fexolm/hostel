use crate::vm::Result;
use std::io::Write as _;

const SERIAL_COM1_BASE: u16 = 0x3f8;
const SERIAL_PORT_COUNT: u16 = 8;
const LCR_DLAB: u8 = 1 << 7;
const LSR_THR_EMPTY: u8 = 1 << 5;
const LSR_TSR_EMPTY: u8 = 1 << 6;

pub struct SerialConsole16550 {
    dll: u8,
    dlm: u8,
    ier: u8,
    lcr: u8,
    mcr: u8,
    scr: u8,
    line_buffer: Vec<u8>,
}

impl SerialConsole16550 {
    pub fn new() -> Self {
        Self {
            dll: 0,
            dlm: 0,
            ier: 0,
            lcr: 0,
            mcr: 0,
            scr: 0,
            line_buffer: Vec::new(),
        }
    }

    pub fn handles_range(&self, port: u16, size: usize) -> bool {
        let Some(last) = port.checked_add(size.saturating_sub(1) as u16) else {
            return false;
        };
        port <= SERIAL_COM1_BASE + SERIAL_PORT_COUNT - 1 && last >= SERIAL_COM1_BASE
    }

    pub fn io_out(&mut self, port: u16, data: &[u8]) -> Result<()> {
        for (idx, &value) in data.iter().enumerate() {
            self.write_reg(port.wrapping_add(idx as u16), value)?;
        }
        Ok(())
    }

    pub fn io_in(&mut self, port: u16, data: &mut [u8]) {
        for (idx, value) in data.iter_mut().enumerate() {
            *value = self.read_reg(port.wrapping_add(idx as u16));
        }
    }

    pub fn flush(&mut self) -> Result<()> {
        if self.line_buffer.is_empty() {
            return Ok(());
        }

        let mut stdout = std::io::stdout().lock();
        stdout.write_all(&self.line_buffer)?;
        stdout.flush()?;
        self.line_buffer.clear();
        Ok(())
    }

    fn write_reg(&mut self, port: u16, value: u8) -> Result<()> {
        let offset = port.wrapping_sub(SERIAL_COM1_BASE);
        match offset {
            0 => {
                if self.lcr & LCR_DLAB != 0 {
                    self.dll = value;
                } else {
                    self.enqueue_tx(value)?;
                }
            }
            1 => {
                if self.lcr & LCR_DLAB != 0 {
                    self.dlm = value;
                } else {
                    self.ier = value;
                }
            }
            2 => {}
            3 => self.lcr = value,
            4 => self.mcr = value,
            7 => self.scr = value,
            _ => {}
        }
        Ok(())
    }

    fn read_reg(&self, port: u16) -> u8 {
        let offset = port.wrapping_sub(SERIAL_COM1_BASE);
        match offset {
            0 => {
                if self.lcr & LCR_DLAB != 0 {
                    self.dll
                } else {
                    0
                }
            }
            1 => {
                if self.lcr & LCR_DLAB != 0 {
                    self.dlm
                } else {
                    self.ier
                }
            }
            2 => 0x01,
            3 => self.lcr,
            4 => self.mcr,
            5 => LSR_THR_EMPTY | LSR_TSR_EMPTY,
            6 => 0xB0,
            7 => self.scr,
            _ => 0xFF,
        }
    }

    fn enqueue_tx(&mut self, value: u8) -> Result<()> {
        if value == b'\r' {
            return Ok(());
        }

        self.line_buffer.push(value);
        if value == b'\n' {
            self.flush()?;
        }
        Ok(())
    }
}
