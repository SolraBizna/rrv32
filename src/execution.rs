use super::{Cpu, ExceptionCause, FloatBits};

#[repr(i32)]
#[derive(Debug)]
pub enum MemoryAccessFailure {
    /// The address was misaligned, and this system doesn't support misaligned
    /// access.
    Unaligned=0,
    /// The address did not point to an actual device.
    Fault=1,
}

pub trait ExecutionEnvironment {
    /// Set to true (default) if the A extension should be supported.
    const SUPPORT_A: bool = true;
    /// Set to true (default) if the C extension should be supported. C support
    /// is DISABLED by default, because our implementation of C requires that
    /// unaligned word reads be supported.
    const SUPPORT_C: bool = false;
    /// Set to true (default) if the M extension should be supported.
    const SUPPORT_M: bool = true;
    /// Return true if the A extension should be enabled (only called if
    /// supported).
    fn enable_a(&self) -> bool { true }
    /// Return true if the C extension should be enabled (only called if
    /// supported).
    fn enable_c(&self) -> bool { true }
    /// Return true if the M extension should be enabled (only called if
    /// supported).
    fn enable_m(&self) -> bool { true }
    /// Return true if the F extension should be enabled (only called if
    /// supported).
    fn enable_f(&self) -> bool { true }
    /// Return true if the D extension should be enabled (only called if
    /// supported).
    fn enable_d(&self) -> bool { true }
    /// Return true if the Q extension should be enabled (only called if
    /// supported).
    fn enable_q(&self) -> bool { true }
    /// Return true if the Zicsr extension should be enabled.
    fn enable_zicsr(&self) -> bool { true }
    /// Return true if the Zifence extension should be enabled.
    fn enable_zifence(&self) -> bool { true }
    /// Read an entire word from memory. `mask` indicates which byte lanes are
    /// active. Return `Err(Unaligned)` if address is not aligned to a four-
    /// byte boundary, **OR** determine and implement unaligned memory access
    /// logic yourself. (See section 2.6 "Load and Store Instructions" of the
    /// RISC-V spec.)
    ///
    /// **NOTE:** In order to support the C extension, your `read_word` must be
    /// able to read a word from a two-byte-aligned address. You may, if you
    /// like, still throw an `Err(Unaligned)` if mask is not all-ones in this
    /// case.
    fn read_word(&mut self, address: u32, mask: u32) -> Result<u32, MemoryAccessFailure>;
    /// Read a halfword from memory. Return `Err(Unaligned)` if address is not
    /// aligned to a four-byte boundary, **OR** determine and implement
    /// unaligned memory access logic yourself.
    fn read_half(&mut self, address: u32) -> Result<u16, MemoryAccessFailure> {
        if address & 1 != 0 { return Err(MemoryAccessFailure::Unaligned) }
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
        if address & 1 != 0 { return Err(MemoryAccessFailure::Unaligned) }
        let lanes = if address & 2 != 0 { 0xFFFF0000 } else { 0x0000FFFF };
        self.write_word(address & !3, (data as u32) << 16 | (data as u32), lanes)
    }
    /// Write a byte to memory. Default implementation calls `write_word` with
    /// an appropriate mask and the value "splatted".
    fn write_byte(&mut self, address: u32, data: u8) -> Result<(), MemoryAccessFailure> {
        let lanes = 0xFF << (address & 3) * 8;
        self.write_word(address & !3, u32::from_ne_bytes([data, data, data, data]), lanes)
    }
    /// Perform an aligned word load from memory, and RESERVE that memory. A
    /// future store conditional should only succeed if that memory is still
    /// reserved and intact.
    ///
    /// If `address` is not aligned to a 4-byte boundary, you should really
    /// consider throwing an alignment fault, even if you otherwise handle
    /// misaligned reads.
    ///
    /// You can set `SUPPORT_A` to false if you don't want to think about this.
    fn load_reserved_word(&mut self, address: u32) -> Result<u32, MemoryAccessFailure>;
    /// Perform an aligned word store from memory, but only succeed if that
    /// memory is still RESERVED (by a matching `load_reserved_word` call).
    ///
    /// If `address` is not aligned to a 4-byte boundary, you should really
    /// consider throwing an alignment fault, even if you otherwise handle
    /// misaligned reads.
    ///
    /// You can set `SUPPORT_A` to false if you don't want to think about this.
    fn store_reserved_word(&mut self, address: u32, data: u32) -> Result<bool, MemoryAccessFailure>;
    /// Respond to an `ECALL` instruction. Default implementation raises an
    /// exception appropriate for `ECALL` in M mode. You may override this to
    /// accelerate operating environment emulation, if you like.
    fn perform_ecall<F: FloatBits>(&mut self, _cpu: &mut Cpu<F>) -> Result<(), (ExceptionCause, u32)> {
        return Err((ExceptionCause::EcallFromMmode,0))
    }
    /// Respond to an `EBREAK` instruction. Default implementation raises an
    /// exception appropriate for `EBREAK`. You may override this to... do
    /// something else?
    fn perform_ebreak<F: FloatBits>(&mut self, _cpu: &mut Cpu<F>) -> Result<(), (ExceptionCause, u32)> {
        return Err((ExceptionCause::Breakpoint,0))
    }
    /// Handle a `CSR*` instruction. Pass the current value to the provided
    /// closure, use the closure's return value as a new value, and return the
    /// old value. The default implementation provides the minimum to make
    /// F/D/Q functional and no others. You should implement those, as well as
    /// `misa` and the timers.
    ///
    /// The default implementation calls `cpu.access_fflags`, `cpu.access_frm`,
    /// and `cpu.access_fcsr` for the floating point status registers, iff
    /// floating point support is activated. You should implement these, as
    /// well as the timing flags shown in table 24.3 "RISC-V control and status
    /// register (CSR) address map" of the RISC-V standard.
    fn csr_access<F: FloatBits>(&mut self, cpu: &mut Cpu<F>, csr_number: u32, handler: impl Fn(u32, u32) -> u32, operand: u32) -> Result<u32, ExceptionCause> {
        if F::SUPPORT_F && self.enable_f() {
            match csr_number {
                0x001 => return cpu.access_fflags(handler, operand),
                0x002 => return cpu.access_frm(handler, operand),
                0x003 => return cpu.access_fcsr(handler, operand),
                _ => (),
            }
        }
        Err(ExceptionCause::IllegalInstruction)
    }
    /// Return true if we should use the slow, exact square root for single
    /// precision, false to use the fast, not completely exact square root.
    fn use_accurate_single_sqrt(&self) -> bool { true }
    /// Return true if we should use the slow, exact square root for double
    /// precision, false to use the fast, not completely exact square root.
    fn use_accurate_double_sqrt(&self) -> bool { true }
    /// Return true if we should use the slow, exact square root for quad
    /// precision, false to use the fast, not completely exact square root.
    /// **NOT CURRENTLY IMPLEMENTED!** Attempting to use the slow quad
    /// precision square root will throw an illegal instruction exception!
    fn use_accurate_quad_sqrt(&self) -> bool { false }
    /// An instruction word has been fetched. Called once per instruction.
    fn account_ifetch(&mut self, _pc: u32) {}
    /// A generic "operation" has been performed.
    fn account_generic_op(&mut self) {}
    /// A memory load was performed. Default implementation calls `memory_op`.
    fn account_memory_load(&mut self, address: u32) { self.account_memory_op(address) }
    /// A memory store was performed. Default implementation calls `memory_op`.
    fn account_memory_store(&mut self, address: u32) { self.account_memory_op(address) }
    /// A doubleword memory load was performed. Default implementation calls
    /// `memory_load` twice.
    fn account_memory_double_load(&mut self, address: u32) { self.account_memory_load(address); self.account_memory_load(address.wrapping_add(4)) }
    /// A doubleword memory store was performed. Default implementation calls
    /// `memory_store` twice.
    fn account_memory_double_store(&mut self, address: u32) { self.account_memory_store(address); self.account_memory_store(address.wrapping_add(4)) }
    /// A quadword memory load was performed. Default implementation calls
    /// `memory_load` four times.
    fn account_memory_quad_load(&mut self, address: u32) { self.account_memory_load(address); self.account_memory_load(address.wrapping_add(4)); self.account_memory_load(address.wrapping_add(8)); self.account_memory_load(address.wrapping_add(12)) }
    /// A quadword memory store was performed. Default implementation calls
    /// `memory_store` four times.
    fn account_memory_quad_store(&mut self, address: u32) { self.account_memory_store(address); self.account_memory_store(address.wrapping_add(4)); self.account_memory_store(address.wrapping_add(8)); self.account_memory_store(address.wrapping_add(12)) }
    /// A memory operation was performed. Called only by the default
    /// implementations of `memory_store` and `memory_load`.
    fn account_memory_op(&mut self, _address: u32) {}
    /// A basic ALU operation has been performed. Default implementation calls
    /// `generic_op`.
    fn account_alu_op(&mut self) { self.account_generic_op() }
    /// A multiplication has been performed. Default implementation calls
    /// `generic_op`.
    fn account_mul_op(&mut self) { self.account_generic_op() }
    /// A division has been performed. Default implementation calls
    /// `generic_op`.
    fn account_div_op(&mut self) { self.account_generic_op() }
    /// An atomic memory access has been performed. Default implementation
    /// calls `generic_op`.
    fn account_amo_op(&mut self) { self.account_generic_op() }
    /// An unconditional jump has been performed. Default implementation
    /// calls `generic_op`.
    fn account_jump_op(&mut self) { self.account_generic_op() }
    /// A conditional branch has been performed. Default implementation
    /// calls `generic_op`.
    fn account_branch_op(&mut self, _did_take: bool, _was_forward: bool) {
        self.account_generic_op()
    }
    /// An ordinary floating point operation has been performed. Default
    /// implementation calls `generic_op`.
    fn account_float_binop(&mut self, _num_words: u32) {
        self.account_generic_op()
    }
    /// A floating point division has been performed. Default implementation
    /// calls `float_binop`.
    fn account_float_divide(&mut self, num_words: u32) {
        self.account_float_binop(num_words)
    }
    /// A three-operand floating point operation has been performed. Default
    /// implementation calls `float_binop`.
    fn account_float_ternop(&mut self, num_words: u32) {
        self.account_float_binop(num_words)
    }
    /// A conversion from int to float has been performed. Default
    /// implementation calls `float_binop`.
    fn account_fcvt_from_int(&mut self, num_words: u32) {
        self.account_float_binop(num_words)
    }
    /// A floating point conversion to int has been performed. Default
    /// implementation calls `float_binop`.
    fn account_fcvt_to_int(&mut self, num_words: u32) {
        self.account_float_binop(num_words)
    }
    /// A floating point square root has been performed. Takes one multiply,
    /// plus one multiply, one divide, and one addition for every iteration.
    /// Default implementation calls `float_binop` once, then `float_divide`
    /// for every iteration.
    fn account_sqrt(&mut self, num_words: u32, num_iterations: u32) {
        self.account_float_binop(num_words);
        for _ in 0 .. num_iterations { self.account_float_divide(num_words) }
    }
}
