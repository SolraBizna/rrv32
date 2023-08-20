use std::io::{Read, Write};

pub struct Memory {
    ram: Vec<u32>,
}

impl Memory {
    pub fn new() -> Memory {
        Memory {
            ram: vec![0; 1 << 22],
        }
    }
    pub fn read_word(&mut self, address: u32) -> u32 {
        if address & 3 != 0 {
            panic!("Unaligned read from {address:08X}");
        }
        if (address as usize) < self.ram.len() << 2 { self.ram[(address >> 2) as usize] }
        else if address == 0xFFFFFFFC {
            let mut buf = [0];
            std::io::stdin().read_exact(&mut buf).expect("EOF");
            buf[0] as u32
        }
        else {
            panic!("Bus error (read from {address:08X}");
        }
    }
    pub fn write_word(&mut self, address: u32, data: u32, mask: u32) {
        if address & 3 != 0 {
            panic!("Unaligned write to {address:08X} of {data:08X}&{mask:08X}");
        }
        if (address as usize) < self.ram.len() << 2 {
            let target = &mut self.ram[(address >> 2) as usize];
            *target = (*target & !mask) | (data & mask);
        }
        else if address == 0xFFFFFFFC {
            std::io::stdout().write_all(&[data as u8]).unwrap();
        }
        else {
            panic!("Bus error (write to {address:08X} of {data:08X}&{mask:08X})");
        }
    }
    pub fn ram(&self) -> &[u32] { &self.ram[..] }
    pub fn ram_mut(&mut self) -> &mut [u32] { &mut self.ram[..] }
}
