use std::{
    ffi::OsString,
    fs::File,
    io::{Read, BufReader, Write},
};

use anyhow::Context;

mod cpu;
use cpu::*;
mod memory;
use memory::*;
mod budget;
use budget::*;

pub struct BoxSpace {
    ram: Vec<u32>,
}

impl BoxSpace {
    pub fn new() -> BoxSpace {
        BoxSpace {
            ram: vec![0; 1 << 23],
        }
    }
    pub fn ram(&self) -> &[u32] { &self.ram[..] }
    pub fn ram_mut(&mut self) -> &mut [u32] { &mut self.ram[..] }
}

impl Memory for BoxSpace {
    fn read_word(&mut self, address: u32, _mask: u32) -> Result<u32, MemoryAccessFailure> {
        if address & 3 != 0 {
            return Err(MemoryAccessFailure::Unaligned)
        }
        let ret =
        if (address as usize) < self.ram.len() << 2 { self.ram[(address >> 2) as usize] }
        else if address == 0xFFFFFFFC {
            let mut buf = [0];
            std::io::stdin().read_exact(&mut buf).expect("EOF");
            buf[0] as u32
        }
        else {
            return Err(MemoryAccessFailure::Fault)
        };
        Ok(ret)
    }
    fn write_word(&mut self, address: u32, data: u32, mask: u32) -> Result<(), MemoryAccessFailure> {
        if address & 3 != 0 {
            return Err(MemoryAccessFailure::Unaligned)
        }
        if (address as usize) < self.ram.len() << 2 {
            let target = &mut self.ram[(address >> 2) as usize];
            *target = (*target & !mask) | (data & mask);
        }
        else if address == 0xFFFFFFFC {
            std::io::stdout().write_all(&[data as u8]).unwrap();
        }
        else {
            return Err(MemoryAccessFailure::Fault)
        }
        Ok(())
    }
}


fn main() {
    let args: Vec<OsString> = std::env::args_os().collect();
    if args.len() != 2 {
        eprintln!("Usage: rv32box path/to/input.txt");
        std::process::exit(1);
    }
    let infile = File::open(&args[1]).context("Unable to open the target file").unwrap();
    let mut memory = BoxSpace::new();
    ipl::initial_program_load(memory.ram_mut(), BufReader::new(infile)).unwrap();
    let mut cpu = Cpu::new();
    loop {
        cpu.step(&mut memory, &mut ());
    }
}

mod ipl {
    use std::io::BufRead;
    use anyhow::{anyhow, Context};
    pub fn initial_program_load<R: BufRead>(buf: &mut [u32], reader: R) -> anyhow::Result<()> {
        let mut lines = reader.lines();
        match lines.next() {
            None => return Err(anyhow!("unexpected eof")),
            Some(Err(x)) => return Err(x.into()),
            Some(Ok(x)) => {
                if x.trim() != "v2.0 raw" {
                    return Err(anyhow!("invalid Logisim memory image header (file must begin with a line \"v2.0 raw\""))
                }
            }
        }
        let mut out_index = 0;
        for line in lines {
            let line = line?;
            let line = line.trim();
            let (count, value) = match line.split_once("*") {
                None => (1, line),
                Some((count, value)) => (count.parse().context("unable to parse count")?, value),
            };
            let value = u32::from_str_radix(&value, 16).context("unable to parse value")?;
            for _ in 0 .. count {
                buf[out_index] = value;
                out_index += 1;
            }
        }
        Ok(())
    }
}