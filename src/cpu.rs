use super::*;

pub struct Cpu {
    registers: [u32; 31],
    pc: u32,
}

fn alu_op(alt: bool, op: u32, a: u32, b: u32) -> u32 {
    match op {
        0b000 => {
            match alt {
                false => a.wrapping_add(b),
                true => a.wrapping_sub(b),
            }
        }
        0b001 => {
            if alt { panic!("illegal alternate ALU op") }
            a << (b & 0b11111)
        }
        0b010 => {
            if alt { panic!("illegal alternate ALU op") }
            if (a as i32) < (b as i32) { 1 } else { 0 }
        }
        0b011 => {
            if alt { panic!("illegal alternate ALU op") }
            if a < b { 1 } else { 0 }
        }
        0b100 => {
            if alt { panic!("illegal alternate ALU op") }
            a ^ b
        }
        0b101 => {
            if alt { ((a as i32) >> (b & 0b11111)) as u32 }
            else { a >> (b & 0b11111) }
        }
        0b110 => {
            if alt { panic!("illegal alternate ALU op") }
            a | b
        }
        0b111 => {
            if alt { panic!("illegal alternate ALU op") }
            a & b
        }
        _ => unreachable!()
    }
}

impl Cpu {
    pub fn new() -> Cpu {
        Cpu {
            registers: [0; 31],
            pc: 0,
        }
    }
    pub fn get_register(&self, index: u32) -> u32 {
        if index >= 1 && index < 32 {
            self.registers[(index - 1) as usize]
        }
        else if index == 0 {
            0
        }
        else {
            panic!("register out of range")
        }
    }
    pub fn put_register(&mut self, index: u32, value: u32) {
        if index >= 1 && index < 32 {
            self.registers[(index - 1) as usize] = value;
        }
        else if index == 0 {
            // do nothing
        }
        else {
            panic!("register out of range")
        }
    }
    pub fn step(&mut self, memory: &mut Memory) {
        let instruction = memory.read_word(self.pc);
        if instruction & 0b11 != 0b11 {
            panic!("16-bit op! {instruction:08X}");
        }
        self.pc = self.pc.wrapping_add(4);
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
                let word = memory.read_word(address & !0b11);
                let result = match funct3!() & 0b11 {
                    0b00 => {
                        let b = word.to_le_bytes()[(address & 0b11) as usize];
                        if sign_extend { b as i8 as u32 }
                        else { b as u32 }
                    }
                    0b01 => {
                        if address & 0b1 != 0 {
                            panic!("Unaligned halfword read of {address:08X}");
                        }
                        let bytes = word.to_le_bytes();
                        let h = if address & 0b10 == 0 {
                            u16::from_le_bytes([bytes[0], bytes[1]])
                        } else {
                            u16::from_le_bytes([bytes[2], bytes[3]])
                        };
                        if sign_extend { h as i16 as u32 }
                        else { h as u32 }
                    }
                    0b10 => {
                        if address & 0b11 != 0 {
                            panic!("Unaligned word read of {address:08X}");
                        }
                        word
                    }
                    _ => {
                        panic!("Illegal load instruction  {instruction:08X}");
                    }
                };
                self.put_register(rd!(), result);
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
                self.put_register(rd!(), alu_op(alt, op, a, b));
            }
            0b00101 => {
                // AUIPC
                self.put_register(rd!(), self.pc.wrapping_add(imm20!()).wrapping_sub(4));
            }
            0b01000 => {
                // STORE
                let base = self.get_register(rs1!());
                let address = base.wrapping_add(imm12s!());
                let word = self.get_register(rs2!());
                match funct3!() {
                    0b000 => {
                        let byte = word as u8;
                        let word = u32::from_le_bytes([byte, byte, byte, byte]);
                        memory.write_word(address & !0b11, word, 0xFF << ((address & 0b11) * 8));
                    }
                    0b001 => {
                        let half = word as u16;
                        let bytes = half.to_le_bytes();
                        let word = u32::from_le_bytes([bytes[0], bytes[1], bytes[0], bytes[1]]);
                        if address & 0b1 != 0 {
                            panic!("Unaligned halfword write of {address:08X}");
                        }
                        if address & 0b10 == 0 {
                            memory.write_word(address & !0b11, word, 0xFFFF);
                        }
                        else {
                            memory.write_word(address & !0b11, word, 0xFFFF0000);
                        }
                    }
                    0b010 => {
                        if address & 0b11 != 0 {
                            panic!("Unaligned word write of {address:08X}");
                        }
                        memory.write_word(address, word, 0xFFFFFFFF);
                    }
                    _ => {
                        panic!("Illegal store instruction  {instruction:08X}");
                    }
                }
            }
            0b01100 => {
                // (OP)
                let alt = match funct7!() {
                    0b0000000 => false,
                    0b0100000 => true,
                    _ => panic!("illegal op funct7 {:07b} for instruction {instruction:08X}", funct7!()),
                };
                let a = self.get_register(rs1!());
                let b = self.get_register(rs2!());
                self.put_register(rd!(), alu_op(alt, funct3!(), a, b));
            }
            0b01101 => {
                // (LUI)
                self.put_register(rd!(), imm20!());
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
                        panic!("illegal BRANCH {instruction:08X}");
                    }
                };
                if should_branch {
                    self.pc = self.pc.wrapping_add(imm_b!()).wrapping_sub(4);
                }
            }
            0b11001 => {
                // JALR
                if funct3!() != 0 {
                    panic!("Invalid JALR {instruction:08X} (nonzero funct3)");
                }
                let offset = imm12!();
                let base = self.get_register(rs1!());
                self.put_register(rd!(), self.pc);
                self.pc = base.wrapping_add(offset) & !1;
            }
            0b11011 => {
                // JAL
                let offset = imm_j!();
                self.put_register(rd!(), self.pc);
                self.pc = self.pc.wrapping_add(offset).wrapping_sub(4);
            }
            0b11100 => {
                unimplemented!("SYSTEM {instruction:08X}");
            }
            _ => {
                panic!("Invalid instruction: {instruction:08X}");
            }
        }
    }
}