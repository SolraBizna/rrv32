use super::{Cpu, ExceptionCause, FloatBits};

/// Reasons that a memory access can fail.
#[repr(i32)]
#[derive(Debug)]
pub enum MemoryAccessFailure {
    /// The address was misaligned, and this system doesn't support misaligned
    /// access.
    Unaligned = 0,
    /// Physical-layer failures: bus errors, PMP violations.
    AccessFault = 1,
    /// Virtual-layer failures.
    PageFault = 2,
}

/// Flags relating to the status of extension registers. (e.g. FS/VS/XS of
/// mstatus)
#[repr(u32)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExtensionStatus {
    /// The extension is disabled.
    Disabled = 0,
    /// The registers are in some OS-defined "initial state". (Set by OS.)
    Initialized = 1,
    /// The registers are not in the "initial state", but they haven't
    /// changed since the last context switch. (Set by OS.)
    Clean = 2,
    /// The registers have changed since the last context switch. (Set by
    /// hardware.)
    Dirty = 3,
}

impl ExtensionStatus {
    /// Convert the enum to raw FS/VS/XS bits. (0b00 = disabled, 0b01 =
    /// initialized, 0b10 = clean, 0b11 = dirty)
    pub fn to_bits(&self) -> u32 {
        match self {
            ExtensionStatus::Disabled => 0,
            ExtensionStatus::Initialized => 1,
            ExtensionStatus::Clean => 2,
            ExtensionStatus::Dirty => 3,
        }
    }
    /// Convert from raw FS/VS/XS bits to an enum value. Bits other than the
    /// two low order bits will be ignored.
    pub fn from_bits(bits: u32) -> ExtensionStatus {
        match bits & 3 {
            0 => ExtensionStatus::Disabled,
            1 => ExtensionStatus::Initialized,
            2 => ExtensionStatus::Clean,
            3 => ExtensionStatus::Dirty,
            _ => unreachable!(),
        }
    }
}

/// Everything *outside* of the core CPU: memory space, CSRs, extension state,
/// cycle accounting. `rrv32` provides the core CPU, you provide one of these,
/// together they make a whole system.
///
/// `ExecutionEnvironment` is not object safe. On a technical level, this is
/// solely because of the `SUPPORT_*` associated constants. The lack of object
/// safety is not concerning, because a `dyn ExecutionEnvironment` would tank
/// `Cpu`'s performance so completely that the crate would be useless anyway.
pub trait ExecutionEnvironment {
    /// Set to true (default) if the A extension should ever be supported. See
    /// also [`enable_a`](Self::enable_a).
    const SUPPORT_A: bool = true;
    /// Set to true (default) if the C extension should ever be supported. See
    /// also [`enable_c`](Self::enable_c).
    const SUPPORT_C: bool = true;
    /// Set to true (default) if the M extension should ever be supported. See
    /// also [`enable_m`](Self::enable_m).
    const SUPPORT_M: bool = true;
    /// Return true if the A extension should be enabled right now, allowing
    /// atomic memory accesses.
    ///
    /// Only checked if [`SUPPORT_A`](Self::SUPPORT_A) is true.
    fn enable_a(&self) -> bool {
        true
    }
    /// Return true if the C extension should be enabled right now, allowing
    /// compressed (16-bit) instructions.
    ///
    /// Only checked if [`SUPPORT_C`](Self::SUPPORT_C) is true.
    fn enable_c(&self) -> bool {
        true
    }
    /// Return true if the M extension should be enabled right now, allowing
    /// multiply/divide instructions.
    ///
    /// Only checked if [`SUPPORT_M`](Self::SUPPORT_M) is true.
    fn enable_m(&self) -> bool {
        true
    }
    /// Return true if the F extension should be enabled right now, allowing
    /// 32-bit floating point calculations.
    ///
    /// Only checked if the chosen [`FloatBits`] is at least `u32`.
    fn enable_f(&self) -> bool {
        true
    }
    /// Return true if the F extension should be enabled right now, allowing
    /// 64-bit floating point calculations.
    ///
    /// Only checked if the chosen [`FloatBits`] is at least `u64`.
    fn enable_d(&self) -> bool {
        true
    }
    /// Return true if the F extension should be enabled right now, allowing
    /// 128-bit floating point calculations.
    ///
    /// Only checked if the chosen [`FloatBits`] is at least `u128`.
    fn enable_q(&self) -> bool {
        true
    }
    /// Return true if the Zicsr extension should be enabled right now,
    /// allowing instructions that read and write CSRs (control and status
    /// registers).
    fn enable_zicsr(&self) -> bool {
        true
    }
    /// Return true if the Zifence extension should be enabled right now,
    /// allowing the `IFENCE` instruction to execute.
    ///
    /// We "implement" `IFENCE` by ignoring it, because we don't emulate an
    /// instruction cache.
    fn enable_zifence(&self) -> bool {
        true
    }
    /// Read an entire word from memory. Return `Err(Unaligned)` if address
    /// is not aligned to a four-byte boundary, **OR** determine and implement
    /// unaligned memory access logic yourself. (See section 2.6 "Load and
    /// Store Instructions" of the RISC-V spec.)
    ///
    /// `mask` indicates which byte lanes are active. All-ones indicates a full
    /// word read. Other values, `0xFFFF0000`, `0x0000FFFF`, `0xFF000000`,
    /// `0x00FF0000`, `0x0000FF00`, and `0x000000FF`, will only be provided if
    /// you use the default implementations of `read_half` and `read_byte`.
    /// **You can usually ignore the `mask` parameter on reads.**
    fn read_word(
        &mut self,
        address: u32,
        mask: u32,
    ) -> Result<u32, MemoryAccessFailure>;
    /// Read one instruction word from memory. If C is enabled, this word might
    /// only be 2-byte aligned. The default implementation calls `read_half`
    /// once or twice for half-aligned instruction fetches. You might be able
    /// to improve on that with a custom implementation, depending on your
    /// memory model. (In particular, if you implement arbitrarily misaligned
    /// word reads, this should just call `read_word` unconditionally.)
    ///
    /// You MAY return `Err(Unaligned)` on unaligned instruction reads, but
    /// `rrv32` should make it impossible to have such a thing. The only ways
    /// to end up in that state:
    ///
    /// - Call `cpu.put_pc` with a half-aligned address while C is disabled.
    /// - Forget the part of the spec that says that disabling C is a no-op if
    ///   the next instruction would be half-aligned.
    ///
    /// And `rrv32` prevents the low bit of the PC from being set under any
    /// circumstances, so you will never get an address that is merely
    /// byte-aligned.
    fn read_instruction(
        &mut self,
        address: u32,
    ) -> Result<u32, MemoryAccessFailure> {
        debug_assert!(address % 2 == 0);
        if address & 2 == 0 {
            // Full word aligned.
            self.read_word(address, !0)
        } else {
            // Half-word aligned.
            let low_bits = self.read_half(address)?;
            if low_bits & 0b11 == 0b11 {
                // 32-bit (or greater) instruction, fetch the upper half.
                let high_bits = self.read_half(address + 2)?;
                Ok(low_bits as u32 | ((high_bits as u32) << 16))
            } else {
                Ok(low_bits as u32)
            }
        }
    }
    /// Read a halfword from memory. Return `Err(Unaligned)` if address is not
    /// aligned to a two-byte boundary, **OR** determine and implement
    /// unaligned memory access logic yourself. Default implementation calls
    /// `read_word` with an appropriate mask.
    fn read_half(&mut self, address: u32) -> Result<u16, MemoryAccessFailure> {
        if address & 1 != 0 {
            return Err(MemoryAccessFailure::Unaligned);
        }
        let lanes = if address & 2 != 0 {
            0xFFFF0000
        } else {
            0x0000FFFF
        };
        let word = self.read_word(address & !3, lanes)?;
        Ok(if address & 2 != 0 {
            (word >> 16) as u16
        } else {
            word as u16
        })
    }
    /// Read a byte from memory. Default implementation calls `read_word` with
    /// an appropriate mask.
    fn read_byte(&mut self, address: u32) -> Result<u8, MemoryAccessFailure> {
        let lanes = 0xFF << ((address & 3) * 8);
        let word = self.read_word(address & !3, lanes)?;
        Ok((word >> ((address & 3) * 8)) as u8)
    }
    /// Write an entire word to memory. `address` is aligned to a four-byte
    /// boundary.
    ///
    /// `mask` indicates which byte lanes are active. All-ones indicates a full
    /// word write. Other values, `0xFFFF0000`, `0x0000FFFF`, `0xFF000000`,
    /// `0x00FF0000`, `0x0000FF00`, and `0x000000FF`, will only be provided if
    /// you use the default implementations of `write_half` and `write_byte`.
    /// If you don't implement `write_half` **and** `write_byte` yourself, you
    /// **must** ensure that only the selected bits change. If you do implement
    /// **both** methods, you can pretend the `mask` parameter doesn't exist.
    ///
    /// (Note that some real memory-mapped devices ignore the byte lane signals
    /// and misbehave amusingly when written with a halfword or byte. Ignoring
    /// `mask` will produce similar misbehavior, if you want.)
    fn write_word(
        &mut self,
        address: u32,
        data: u32,
        mask: u32,
    ) -> Result<(), MemoryAccessFailure>;
    /// Write a halfword to memory. Return `Err(Unaligned)` if address is not
    /// aligned to a two-byte boundary, **OR** determine and implement
    /// unaligned memory access logic yourself. Default implementation calls
    /// `write_word` with an appropriate mask and the value "splatted".
    fn write_half(
        &mut self,
        address: u32,
        data: u16,
    ) -> Result<(), MemoryAccessFailure> {
        if address & 1 != 0 {
            return Err(MemoryAccessFailure::Unaligned);
        }
        let lanes = if address & 2 != 0 {
            0xFFFF0000
        } else {
            0x0000FFFF
        };
        self.write_word(
            address & !3,
            (data as u32) << 16 | (data as u32),
            lanes,
        )
    }
    /// Write a byte to memory. Default implementation calls `write_word` with
    /// an appropriate mask and the value "splatted".
    fn write_byte(
        &mut self,
        address: u32,
        data: u8,
    ) -> Result<(), MemoryAccessFailure> {
        let lanes = 0xFF << ((address & 3) * 8);
        self.write_word(
            address & !3,
            u32::from_ne_bytes([data, data, data, data]),
            lanes,
        )
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
    fn load_reserved_word(
        &mut self,
        address: u32,
    ) -> Result<u32, MemoryAccessFailure>;
    /// Perform an aligned word store from memory, but only succeed if that
    /// memory is still RESERVED (by a matching `load_reserved_word` call)
    /// **and** has not been touched by another write since it was reserved.
    ///
    /// If `address` is not aligned to a 4-byte boundary, you should really
    /// consider throwing an alignment fault, even if you otherwise handle
    /// misaligned reads.
    ///
    /// You can set `SUPPORT_A` to false if you don't want to think about this.
    fn store_reserved_word(
        &mut self,
        address: u32,
        data: u32,
    ) -> Result<bool, MemoryAccessFailure>;
    /// Respond to an `ECALL` instruction. Default implementation raises an
    /// exception appropriate for `ECALL` in M mode. You may override this to
    /// accelerate operating environment emulation, if you like.
    fn perform_ecall<F: FloatBits>(
        &mut self,
        _cpu: &mut Cpu<F>,
    ) -> Result<(), (ExceptionCause, u32)> {
        Err((ExceptionCause::EcallFromMmode, 0))
    }
    /// Respond to an `EBREAK` instruction. Default implementation raises an
    /// exception appropriate for `EBREAK`. You may override this to... do
    /// something else?
    fn perform_ebreak<F: FloatBits>(
        &mut self,
        _cpu: &mut Cpu<F>,
    ) -> Result<(), (ExceptionCause, u32)> {
        Err((ExceptionCause::Breakpoint, 0))
    }
    /// Read from a CSR. Return `Err(IllegalInstruction)` if the CSR number is
    /// not recognized.
    ///
    /// The CPU implements the floating point status registers, if floating
    /// point is enabled. All other CSRs are your responsibility. The default
    /// implementation just returns `Err(IllegalInstruction)`.
    fn read_csr(&mut self, _csr_number: u32) -> Result<u32, ExceptionCause> {
        Err(ExceptionCause::IllegalInstruction)
    }
    /// Write to a CSR. Return `Err(IllegalInstruction)` if the CSR number is
    /// not recognized.
    ///
    /// The CPU implements the floating point status registers, if floating
    /// point is enabled. All other CSRs are your responsibility. The default
    /// implementation just returns `Err(IllegalInstruction)`.
    fn write_csr(
        &mut self,
        _csr_number: u32,
        _new_value: u32,
    ) -> Result<(), ExceptionCause> {
        Err(ExceptionCause::IllegalInstruction)
    }
    /// Return the status of the floating point registers. The default
    /// implementation just returns Dirty all the time, which makes context
    /// switches potentially a little less efficient in an OS environment.
    fn read_fs(&self) -> ExtensionStatus {
        ExtensionStatus::Dirty
    }
    /// Set the status of the floating point registers. The OS may set the
    /// status to Disabled, Initialized, or Clean during a context switch. The
    /// hardware may set the status to Dirty in response to a floating point
    /// operation.
    fn write_fs(&mut self, _status: ExtensionStatus) {}
    /// Return true if we should use the slow, exact square root for single
    /// precision at the moment, false to use the fast, not completely exact
    /// square root. Default is true (slow, exact).
    fn use_accurate_single_sqrt(&self) -> bool {
        true
    }
    /// Return true if we should use the slow, exact square root for double
    /// precision at the moment, false to use the fast, not completely exact
    /// square root. Default is true (slow, exact).
    fn use_accurate_double_sqrt(&self) -> bool {
        true
    }
    /// Return true if we should use the slow, exact square root for quad
    /// precision at the moment, false to use the fast, not completely exact
    /// square root. **NOT CURRENTLY IMPLEMENTED!** Attempting to use the slow
    /// quad precision square root will throw an illegal instruction exception!
    /// This is because the `ieee-apsqrt` crate, which we use to implement the
    /// `SQRT` instruction, does not currently have an implementation of exact
    /// sqrt for quad-precision. Relatedly, this flag defaults to *false* and
    /// will become true if and only if `ieee-apsqrt` adds support for exact
    /// quad-precision sqrt (or we switch to another sqrt implementation that
    /// has it).
    fn use_accurate_quad_sqrt(&self) -> bool {
        false
    }
    /// An instruction word has been fetched. Called once per instruction.
    fn account_ifetch(&mut self, _pc: u32) {}
    /// A generic "operation" has been performed.
    fn account_generic_op(&mut self) {}
    /// A memory load was performed. Default implementation calls
    /// [`memory_op`](Self::account_memory_op).
    fn account_memory_load(&mut self, address: u32) {
        self.account_memory_op(address)
    }
    /// A memory store was performed. Default implementation calls
    /// [`memory_op`](Self::account_memory_op).
    fn account_memory_store(&mut self, address: u32) {
        self.account_memory_op(address)
    }
    /// A doubleword memory load was performed. Default implementation calls
    /// [`memory_load`](Self::account_memory_load) twice.
    fn account_memory_double_load(&mut self, address: u32) {
        self.account_memory_load(address);
        self.account_memory_load(address.wrapping_add(4))
    }
    /// A doubleword memory store was performed. Default implementation calls
    /// [`memory_store`](Self::account_memory_store) twice.
    fn account_memory_double_store(&mut self, address: u32) {
        self.account_memory_store(address);
        self.account_memory_store(address.wrapping_add(4))
    }
    /// A quadword memory load was performed. Default implementation calls
    /// [`memory_load`](Self::account_memory_load) four times.
    fn account_memory_quad_load(&mut self, address: u32) {
        self.account_memory_load(address);
        self.account_memory_load(address.wrapping_add(4));
        self.account_memory_load(address.wrapping_add(8));
        self.account_memory_load(address.wrapping_add(12))
    }
    /// A quadword memory store was performed. Default implementation calls
    /// [`memory_store`](Self::account_memory_store) four times.
    fn account_memory_quad_store(&mut self, address: u32) {
        self.account_memory_store(address);
        self.account_memory_store(address.wrapping_add(4));
        self.account_memory_store(address.wrapping_add(8));
        self.account_memory_store(address.wrapping_add(12))
    }
    /// A memory operation was performed. Called only by the default
    /// implementations of [`memory_store`](Self::account_memory_store)
    /// and [`memory_load`](Self::account_memory_load).
    fn account_memory_op(&mut self, _address: u32) {}
    /// A basic ALU operation has been performed. Default implementation calls
    /// [`generic_op`](Self::account_generic_op).
    fn account_alu_op(&mut self) {
        self.account_generic_op()
    }
    /// A multiplication has been performed. Default implementation calls
    /// [`alu_op`](Self::account_alu_op).
    fn account_mul_op(&mut self) {
        self.account_alu_op()
    }
    /// A division has been performed. Default implementation calls
    /// [`alu_op`](Self::account_alu_op).
    fn account_div_op(&mut self) {
        self.account_alu_op()
    }
    /// An atomic memory access has been performed. Default implementation
    /// calls [`generic_op`](Self::account_generic_op).
    fn account_amo_op(&mut self) {
        self.account_generic_op()
    }
    /// An unconditional jump has been performed. Default implementation
    /// calls [`generic_op`](Self::account_generic_op).
    fn account_jump_op(&mut self) {
        self.account_generic_op()
    }
    /// A conditional branch has been performed. Default implementation
    /// calls [`generic_op`](Self::account_generic_op).
    fn account_branch_op(&mut self, _did_take: bool, _was_forward: bool) {
        self.account_generic_op()
    }
    /// An ordinary floating point operation has been performed. Default
    /// implementation calls [`generic_op`](Self::account_generic_op).
    fn account_float_op(&mut self, _num_words: u32) {
        self.account_generic_op()
    }
    /// A floating point division has been performed. Default implementation
    /// calls [`float_op`](Self::account_float_op).
    fn account_float_divide(&mut self, num_words: u32) {
        self.account_float_op(num_words)
    }
    /// A three-operand floating point operation has been performed. Default
    /// implementation calls [`float_op`](Self::account_float_op).
    fn account_float_ternop(&mut self, num_words: u32) {
        self.account_float_op(num_words)
    }
    /// A conversion from int to float has been performed. Default
    /// implementation calls [`float_op`](Self::account_float_op).
    fn account_fcvt_from_int(&mut self, num_words: u32) {
        self.account_float_op(num_words)
    }
    /// A floating point conversion to int has been performed. Default
    /// implementation calls [`float_op`](Self::account_float_op).
    fn account_fcvt_to_int(&mut self, num_words: u32) {
        self.account_float_op(num_words)
    }
    /// A floating point square root has been performed. Takes one multiply,
    /// plus one multiply, one divide, and one addition for every iteration.
    /// Default implementation calls [`float_op`](Self::account_float_op)
    /// once, then [`float_divide`](Self::account_float_divide) for every
    /// iteration.
    fn account_sqrt(&mut self, num_words: u32, num_iterations: u32) {
        self.account_float_op(num_words);
        for _ in 0..num_iterations {
            self.account_float_divide(num_words)
        }
    }
}
