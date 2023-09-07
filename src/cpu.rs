use super::*;

/// Exceptions that can occur during execution of an instruction. Values
/// correspond to `mcause` values.
#[repr(i32)]
#[derive(Debug)]
#[allow(unused)]
pub enum MachineException {
    MisalignedPC=0,
    InstructionFault=1,
    IllegalInstruction=2,
    Breakpoint=3,
    MisalignedLoad=4,
    LoadFault=5,
    MisalignedStore=6,
    StoreFault=7,
    EcallFromUmode=8,
    EcallFromSmode=9,
    EcallFromMmode=11,
    InstructionPageFault=12,
    LoadPageFault=13,
    StorePageFault=15,
}

#[repr(C)]
pub struct Cpu {
    registers: [u32; 32], // pc is stored where x0 would be
}

fn alu_op(alt: bool, op: u32, a: u32, b: u32) -> Result<u32,MachineException> {
    Ok(match op {
        0b000 => {
            match alt {
                false => a.wrapping_add(b),
                true => a.wrapping_sub(b),
            }
        }
        0b001 => {
            if alt { return Err(MachineException::IllegalInstruction) }
            a << (b & 0b11111)
        }
        0b010 => {
            if alt { return Err(MachineException::IllegalInstruction) }
            if (a as i32) < (b as i32) { 1 } else { 0 }
        }
        0b011 => {
            if alt { return Err(MachineException::IllegalInstruction) }
            if a < b { 1 } else { 0 }
        }
        0b100 => {
            if alt { return Err(MachineException::IllegalInstruction) }
            a ^ b
        }
        0b101 => {
            if alt { ((a as i32) >> (b & 0b11111)) as u32 }
            else { a >> (b & 0b11111) }
        }
        0b110 => {
            if alt { return Err(MachineException::IllegalInstruction) }
            a | b
        }
        0b111 => {
            if alt { return Err(MachineException::IllegalInstruction) }
            a & b
        }
        _ => unreachable!()
    })
}

impl Cpu {
    pub fn get_pc(&self) -> u32 { return self.registers[0] }
    pub fn put_pc(&mut self, new_pc: u32) { self.registers[0] = new_pc & !1; }
    pub fn new() -> Cpu {
        Cpu {
            registers: [0; 32],
        }
    }
    pub fn get_register(&self, index: u32) -> u32 {
        if index >= 1 && index < 32 {
            self.registers[index as usize]
        }
        else if index == 0 {
            0
        }
        else {
            panic!("register {index} out of range")
        }
    }
    pub fn put_register(&mut self, index: u32, value: u32) {
        if index >= 1 && index < 32 {
            self.registers[index as usize] = value;
        }
        else if index == 0 {
            // do nothing
        }
        else {
            panic!("register {index} out of range")
        }
    }
    fn internal_step<M: Memory, B: Budget>(&mut self, memory: &mut M, budget: &mut B) -> Result<(), MachineException> {
        let this_pc = self.get_pc();
        let instruction = memory.read_word(this_pc, !0)
            .map_err(ifetch_exception)?;
        if instruction & 0b11 != 0b11 {
            return Err(MachineException::IllegalInstruction)
        }
        budget.ifetch(this_pc);
        let mut next_pc = this_pc.wrapping_add(4);
        let opcode = (instruction >> 2) & 0b11111;
        // I don't want to calculate these when they're not used, but I don't
        // want to repeat myself either. Fortunately for me, yesterday I
        // learned that Rust's macro identifier hygiene rules includes lexical
        // scope!
        macro_rules! funct3 { () => { (instruction >> 12) & 0b111 }; }
        macro_rules! funct7 { () => { (instruction >> 25) & 0b1111111 }; }
        macro_rules! rs1 { () => { (instruction >> 15) & 0b11111 }; }
        macro_rules! rs2 { () => { (instruction >> 20) & 0b11111 }; }
        macro_rules! rd { () => { (instruction >> 7) & 0b11111 }; }
        macro_rules! imm12 { () => { ((instruction as i32) >> 20) as u32 }; }
        macro_rules! imm12s { () => {
            (((instruction as i32) >> 20) as u32 & !0b11111)
            | (((instruction as i32) >> 7) as u32 & 0b11111)
        }; }
        macro_rules! imm20 { () => { instruction & 0xFFFFF000 }; }
        macro_rules! imm_j { () => {
            {
                let imm_10_1 = (instruction >> 21) & 0b1111111111;
                let imm_11 = (instruction >> 20) & 0b1;
                let imm_19_12 = (instruction >> 12) & 0b11111111;
                let imm_20 = (instruction as i32) >> 31;
                (imm_10_1 << 1)
                | (imm_11 << 11)
                | (imm_19_12 << 12)
                | (imm_20 << 20) as u32
            }
        };}
        macro_rules! imm_b { () => {
            {
                let imm_4_1 = (instruction >> 8) & 0b1111;
                let imm_10_5 = (instruction >> 25) & 0b111111;
                let imm_11 = (instruction >> 7) & 0b1;
                let imm_12 = ((instruction as i32) >> 31);
                (imm_4_1 << 1)
                | (imm_10_5 << 5)
                | (imm_11 << 11)
                | (imm_12 << 12) as u32
            }
        };}
        match opcode {
            0b00000 => {
                // LOAD
                let sign_extend = funct3!() & 0b100 == 0;
                let base = self.get_register(rs1!());
                let address = base.wrapping_add(imm12!());
                let result = match funct3!() & 0b11 {
                    0b00 => {
                        let b = memory.read_byte(address).map_err(load_exception)?;
                        if sign_extend { b as i8 as u32 }
                        else { b as u32 }
                    }
                    0b01 => {
                        let h = memory.read_half(address).map_err(load_exception)?;
                        if sign_extend { h as i16 as u32 }
                        else { h as u32 }
                    }
                    0b10 => {
                        memory.read_word(address, !0).map_err(load_exception)?
                    }
                    _ => {
                        return Err(MachineException::IllegalInstruction)
                    }
                };
                self.put_register(rd!(), result);
                budget.memory_load(address);
            }
            0b00011 => {
                //unimplemented!("MISC-MEM {instruction:08X}");
            }
            0b00100 => {
                // OP-IMM
                let op = funct3!();
                let alt = op == 0b101 && (instruction & (1 << 30)) != 0;
                let a = self.get_register(rs1!());
                let b = imm12!();
                self.put_register(rd!(), alu_op(alt, op, a, b)?);
                budget.alu_op();
            }
            0b00101 => {
                // AUIPC
                self.put_register(rd!(), this_pc.wrapping_add(imm20!()));
                budget.alu_op();
            }
            0b01000 => {
                // STORE
                let base = self.get_register(rs1!());
                let address = base.wrapping_add(imm12s!());
                let word = self.get_register(rs2!());
                match funct3!() {
                    0b000 =>
                        memory.write_byte(address, word as u8)
                            .map_err(store_exception)?,
                    0b001 =>
                        memory.write_half(address, word as u16)
                            .map_err(store_exception)?,
                    0b010 =>
                        memory.write_word(address, word, 0xFFFFFFFF)
                            .map_err(store_exception)?,
                    _ => {
                        return Err(MachineException::IllegalInstruction)
                    }
                }
                budget.memory_store(address);
            }
            0b01100 => {
                // (OP)
                let alt = match funct7!() {
                    0b0000000 => false,
                    0b0100000 => true,
                    _ => return Err(MachineException::IllegalInstruction)
                };
                let a = self.get_register(rs1!());
                let b = self.get_register(rs2!());
                self.put_register(rd!(), alu_op(alt, funct3!(), a, b)?);
                budget.alu_op();
            }
            0b01101 => {
                // (LUI)
                self.put_register(rd!(), imm20!());
                budget.generic_op();
            }
            0b11000 => {
                // (BRANCH)
                let a = self.get_register(rs1!());
                let b = self.get_register(rs2!());
                let should_branch = match funct3!() {
                    0b000 => a == b,
                    0b001 => a != b,
                    0b100 => (a as i32) < (b as i32),
                    0b101 => (a as i32) >= (b as i32),
                    0b110 => a < b,
                    0b111 => a >= b,
                    _ => {
                        return Err(MachineException::IllegalInstruction)
                    }
                };
                if should_branch {
                    next_pc = this_pc.wrapping_add(imm_b!());
                }
                budget.branch(should_branch);
            }
            0b11001 => {
                // JALR
                if funct3!() != 0 {
                    return Err(MachineException::IllegalInstruction)
                }
                let offset = imm12!();
                let base = self.get_register(rs1!());
                self.put_register(rd!(), next_pc);
                next_pc = base.wrapping_add(offset) & !1;
                budget.jump();
            }
            0b11011 => {
                // JAL
                let offset = imm_j!();
                self.put_register(rd!(), next_pc);
                next_pc = this_pc.wrapping_add(offset);
                budget.jump();
            }
            0b11100 => {
                unimplemented!("SYSTEM {instruction:08X}");
            }
            _ => {
                return Err(MachineException::IllegalInstruction)
            }
        }
        self.put_pc(next_pc);
        Ok(())
    }
    pub fn step<M: Memory, B: Budget>(&mut self, memory: &mut M, budget: &mut B) {
        self.internal_step(memory, budget).unwrap()
    }
}

// Convert from memory exceptions to the appropriate `MachineException`
fn ifetch_exception(e: MemoryAccessFailure) -> MachineException {
    match e {
        MemoryAccessFailure::Unaligned => MachineException::MisalignedPC,
        MemoryAccessFailure::Fault => MachineException::InstructionFault,
    }
}
fn load_exception(e: MemoryAccessFailure) -> MachineException {
    match e {
        MemoryAccessFailure::Unaligned => MachineException::MisalignedLoad,
        MemoryAccessFailure::Fault => MachineException::LoadFault,
    }
}
fn store_exception(e: MemoryAccessFailure) -> MachineException {
    match e {
        MemoryAccessFailure::Unaligned => MachineException::MisalignedStore,
        MemoryAccessFailure::Fault => MachineException::StoreFault,
    }
}