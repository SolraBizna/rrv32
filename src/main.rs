use std::{
    ffi::OsString,
    fs::File,
    io::BufReader,
};

use anyhow::Context;

mod cpu;
use cpu::*;
mod ipl;
mod memory;
use memory::*;

fn main() {
    let args: Vec<OsString> = std::env::args_os().collect();
    if args.len() != 2 {
        eprintln!("Usage: rv32box path/to/input.txt");
        std::process::exit(1);
    }
    let infile = File::open(&args[1]).context("Unable to open the target file").unwrap();
    let mut memory = Memory::new();
    ipl::initial_program_load(memory.ram_mut(), BufReader::new(infile)).unwrap();
    let mut cpu = Cpu::new();
    loop {
        cpu.step(&mut memory);
    }
}
