#[repr(i32)]
#[derive(Debug)]
pub enum MemoryAccessFailure {
    /// The address was misaligned, and this system doesn't support misaligned
    /// access.
    Unaligned=0,
    /// The address did not point to an actual device.
    Fault=1,
}

pub trait Memory {
    /// Read an entire word from memory. `mask` indicates which byte lanes are
    /// active. Return `Err(Unaligned)` if address is not aligned to a four-
    /// byte boundary, **OR** determine and implement unaligned memory access
    /// logic yourself.
    fn read_word(&mut self, address: u32, mask: u32) -> Result<u32, MemoryAccessFailure>;
    /// Read a halfword from memory. Return `Err(Unaligned)` if address is not
    /// aligned to a four-byte boundary, **OR** determine and implement
    /// unaligned memory access logic yourself.
    fn read_half(&mut self, address: u32) -> Result<u16, MemoryAccessFailure> {
        if address & 2 != 0 { return Err(MemoryAccessFailure::Unaligned) }
        let lanes = if address & 2 != 0 { 0xFFFF0000 } else { 0x0000FFFF };
        let word = self.read_word(address & !3, lanes)?;
        Ok(if address & 2 != 0 { (word >> 16) as u16 } else { word as u16 })
    }
    /// Read a byte from memory. Default implementation calls `read_word` with
    /// an appropriate mask.
    fn read_byte(&mut self, address: u32) -> Result<u8, MemoryAccessFailure> {
        let lanes = 0xFF << ((address & 3) * 8);
        let word = self.read_word(address & !3, lanes)?;
        Ok((word >> (address & 3) * 8) as u8)
    }
    /// Write an entire word to memory. `address` is aligned to a four-byte
    /// boundary. `mask` indicates which byte lanes are active.
    fn write_word(&mut self, address: u32, data: u32, mask: u32) -> Result<(), MemoryAccessFailure>;
    /// Write a halfword to memory. `address` is aligned to a two-byte
    /// boundary. Default implementation calls `write_word` with an appropriate
    /// mask and the value "splatted".
    fn write_half(&mut self, address: u32, data: u16) -> Result<(), MemoryAccessFailure> {
        if address & 2 != 0 { return Err(MemoryAccessFailure::Unaligned) }
        let lanes = if address & 2 != 0 { 0xFFFF0000 } else { 0x0000FFFF };
        self.write_word(address & !3, (data as u32) << 16 | (data as u32), lanes)
    }
    /// Write a byte to memory. Default implementation calls `write_word` with
    /// an appropriate mask and the value "splatted".
    fn write_byte(&mut self, address: u32, data: u8) -> Result<(), MemoryAccessFailure> {
        let lanes = 0xFF << (address & 3) * 8;
        self.write_word(address & !3, u32::from_ne_bytes([data, data, data, data]), lanes)
    }
}
