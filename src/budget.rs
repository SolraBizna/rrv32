/// Represents some means of accounting CPU time. Useful for throttling and
/// profiling. `()` implements a simple no-op accounting scheme.
pub trait Budget {
    /// An instruction word has been fetched. Called once per instruction.
    fn ifetch(&mut self, pc: u32);
    /// A generic "operation" has been performed.
    fn generic_op(&mut self);
    /// A memory load was performed. Default implementation calls `memory_op`.
    fn memory_load(&mut self, address: u32) { self.memory_op(address) }
    /// A memory store was performed. Default implementation calls `memory_op`.
    fn memory_store(&mut self, address: u32) { self.memory_op(address) }
    /// A memory operation was performed. Called only by the default
    /// implementations of `memory_store` and `memory_load`.
    fn memory_op(&mut self, address: u32);
    /// A basic ALU operation has been performed. Default implementation calls
    /// `generic_op`.
    fn alu_op(&mut self) { self.generic_op() }
    /// An atomic memory access has been performed. Default implementation
    /// calls `generic_op`.
    fn amo_op(&mut self) { self.generic_op() }
    /// An unconditional jump has been performed. Default implementation
    /// calls `generic_op`.
    fn jump(&mut self) { self.generic_op() }
    /// A conditional branch has been performed. Default implementation
    /// calls `generic_op`.
    fn branch(&mut self, did_take: bool) {
        let _ = did_take;
        self.generic_op()
    }
    /// An exception has taken place. Default implementation calls
    // `generic_op`.
    fn exception(&mut self) { self.generic_op() }
}

impl Budget for () {
    fn ifetch(&mut self, _pc: u32) {}
    fn generic_op(&mut self) {}
    fn memory_op(&mut self, _address: u32) {}
}