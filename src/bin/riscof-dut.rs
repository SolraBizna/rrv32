// Hilights of this file: rampant unwrapping, fragility, assumptions...

use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
};

use rrv32::{Cpu, ExceptionCause, ExecutionEnvironment, FloatBits, MemoryAccessFailure};

fn print_usage_and_exit(fatal: bool) {
    println!("Usage: riscof-dut --isa=imafdq --signature-path=PATH --exe-path=PATH");
    std::process::exit(if fatal { 1 } else { 0 })
}

#[derive(Debug)]
enum FloatISA { None, F, D, Q }

fn parse_args() -> (String, String, String) {
    let mut isa = None;
    let mut signature_path = None;
    let mut exe_path = None;
    for arg in std::env::args().skip(1) {
        if let Some((lhs, rhs)) = arg.split_once("=") {
            match lhs {
                "--isa" => isa = Some(rhs.to_string()),
                "--signature-path" => signature_path = Some(rhs.to_string()),
                "--exe-path" => exe_path = Some(rhs.to_string()),
                "--signature-granularity" => {
                    if rhs != "4" {
                        println!("Only supported value for signature-granularity is 4.");
                        std::process::exit(1);
                    }
                },
                _ => {
                    println!("Unknown parameter {lhs:?}");
                    print_usage_and_exit(true);
                },
            }
        } else {
            match arg.as_str() {
                "--isa" | "--signature-path" | "--exe-path" | "--signature-granularity" => {
                    println!("{arg} requires an equals sign and an argument");
                    print_usage_and_exit(true);
                },
                "help" | "--help" | "-h" | "-?" => {
                    print_usage_and_exit(false);
                },
                _ => {
                    if arg.starts_with("-") {
                        println!("Unknown option {arg:?}");
                    } else {
                        println!("Unexpected bare parameter {arg:?}.");
                    }
                    print_usage_and_exit(true);
                }
            }
        }
    }
    if isa.is_none() || signature_path.is_none() || exe_path.is_none() {
        if isa.is_none() { println!("Missing parameter: --isa"); }
        if signature_path.is_none() { println!("Missing parameter: --signature-path"); }
        if exe_path.is_none() { println!("Missing parameter: --exe-path"); }
        print_usage_and_exit(true);
    }
    return (isa.unwrap(), signature_path.unwrap(), exe_path.unwrap());
}

#[allow(unused)]
struct ElfHeader {
    e_ident: [u8; 16],
    e_type: u16,
    e_machine: u16,
    e_version: u32,
    e_entry: u32, // Elf32_Addr
    e_phoff: u32, // Elf32_Off
    e_shoff: u32, // Elf32_Off
    e_flags: u32,
    e_ehsize: u16,
    e_phentsize: u16,
    e_phnum: u16,
    e_shentsize: u16,
    e_shnum: u16,
    e_shstrndx: u16,
}

fn read_elf_header(file: &mut File) -> ElfHeader {
    let mut buf = [0u8; 52];
    file.read_exact(&mut buf).unwrap();
    assert_eq!(&buf[0..4], b"\x7FELF", "not an ELF header");
    assert_eq!(buf[4], 0x01, "not a 32-bit ELF");
    assert_eq!(buf[5], 0x01, "not a two's complement little-endian ELF");
    assert_eq!(buf[6], 0x01, "not a version 1 ELF file");
    // ignore 7-8, assume valid ABI
    // ignore 9-15, they are reserved and should be ignored if not understood
    let e_type = u16::from_le_bytes([buf[16], buf[17]]);
    assert_eq!(e_type, 2, "Not an executable ELF!");
    let e_machine = u16::from_le_bytes([buf[18], buf[19]]);
    assert_eq!(e_machine, 243, "Not a RISC-V ELF!");
    let e_version = u32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]);
    let e_entry = u32::from_le_bytes([buf[24], buf[25], buf[26], buf[27]]);
    let e_phoff = u32::from_le_bytes([buf[28], buf[29], buf[30], buf[31]]);
    let e_shoff = u32::from_le_bytes([buf[32], buf[33], buf[34], buf[35]]);
    let e_flags = u32::from_le_bytes([buf[36], buf[37], buf[38], buf[39]]);
    let e_ehsize = u16::from_le_bytes([buf[40], buf[41]]);
    assert!(e_ehsize >= 52, "Main header in ELF too small!");
    let e_phentsize = u16::from_le_bytes([buf[42], buf[43]]);
    let e_phnum = u16::from_le_bytes([buf[44], buf[45]]);
    let e_shentsize = u16::from_le_bytes([buf[46], buf[47]]);
    let e_shnum = u16::from_le_bytes([buf[48], buf[49]]);
    let e_shstrndx = u16::from_le_bytes([buf[50], buf[51]]);
    ElfHeader {
        e_ident: buf[0..16].try_into().unwrap(),
        e_type,
        e_machine,
        e_version,
        e_entry,
        e_phoff,
        e_shoff,
        e_flags,
        e_ehsize,
        e_phentsize,
        e_phnum,
        e_shentsize,
        e_shnum,
        e_shstrndx,
    }
}

#[allow(unused)]
struct ElfProgramHeader {
    p_type: u32,
    p_offset: u32, // Elf32_Off
    p_vaddr: u32, // Elf32_Addr
    p_paddr: u32, // Elf32_Addr
    p_filesz: u32,
    p_memsz: u32,
    p_flags: u32,
    p_align: u32,
}

fn read_elf_program_header(file: &mut File) -> ElfProgramHeader {
    let mut buf = [0u8; 32];
    file.read_exact(&mut buf).unwrap();
    let p_type = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    let p_offset = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
    let p_vaddr = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
    let p_paddr = u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]);
    let p_filesz = u32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]);
    let p_memsz = u32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]);
    let p_flags = u32::from_le_bytes([buf[24], buf[25], buf[26], buf[27]]);
    let p_align = u32::from_le_bytes([buf[28], buf[29], buf[30], buf[31]]);
    ElfProgramHeader {
        p_type,
        p_offset,
        p_vaddr,
        p_paddr,
        p_filesz,
        p_memsz,
        p_flags,
        p_align,
    }
}

#[allow(unused)]
struct ElfSectionHeader {
    sh_name: u32,
    sh_type: u32,
    sh_flags: u32,
    sh_addr: u32, // Elf32_Addr
    sh_offset: u32, // Elf32_Off
    sh_size: u32,
    sh_link: u32,
    sh_info: u32,
    sh_addralign: u32,
    sh_entsize: u32,
}

fn read_elf_section_header(file: &mut File) -> ElfSectionHeader {
    let mut buf = [0u8; 40];
    file.read_exact(&mut buf).unwrap();
    let sh_name = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    let sh_type = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
    let sh_flags = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
    let sh_addr = u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]);
    let sh_offset = u32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]);
    let sh_size = u32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]);
    let sh_link = u32::from_le_bytes([buf[24], buf[25], buf[26], buf[27]]);
    let sh_info = u32::from_le_bytes([buf[28], buf[29], buf[30], buf[31]]);
    let sh_addralign = u32::from_le_bytes([buf[32], buf[33], buf[34], buf[35]]);
    let sh_entsize = u32::from_le_bytes([buf[36], buf[37], buf[38], buf[39]]);
    ElfSectionHeader {
        sh_name,
        sh_type,
        sh_flags,
        sh_addr,
        sh_offset,
        sh_size,
        sh_link,
        sh_info,
        sh_addralign,
        sh_entsize,
    }
}

#[allow(unused)]
struct ElfSymbol {
    st_name: u32,
    st_value: u32, // Elf32_Addr
    st_size: u32,
    st_info: u8,
    st_other: u8,
    st_shndx: u16,
}

fn read_elf_symbol(file: &mut File) -> ElfSymbol {
    let mut buf = [0u8; 16];
    file.read_exact(&mut buf).unwrap();
    let st_name = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    let st_value = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
    let st_size = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
    let st_info = buf[12];
    let st_other = buf[13];
    let st_shndx = u16::from_le_bytes([buf[14], buf[15]]);
    ElfSymbol {
        st_name,
        st_value,
        st_size,
        st_info,
        st_other,
        st_shndx,
    }
}

struct LoadedElf {
    sections: Vec<LoadedChunk>,
    entry_point: u32,
    symbol_table: HashMap<Vec<u8>, u32>,
}

struct LoadedChunk {
    base: u32,
    words: Vec<u32>,
}

fn load_elf(path: &str) -> LoadedElf {
    let mut f = File::open(path).unwrap();
    let header = read_elf_header(&mut f);
    assert_ne!(header.e_phoff, 0, "No program headers in ELF!");
    assert_eq!(header.e_phentsize, 32, "Program headers in ELF not 32 bytes long!");
    let chunks = (0 .. header.e_phnum).filter_map(|n| {
        f.seek(SeekFrom::Start((header.e_phoff + header.e_phentsize as u32 * n as u32) as u64)).unwrap();
        let program_header = read_elf_program_header(&mut f);
        if program_header.p_type != 1 { return None } //only care about PT_LOAD
        assert_eq!(program_header.p_vaddr, program_header.p_paddr, "ELF seems to assume virtual memory?");
        assert!(program_header.p_filesz <= program_header.p_memsz, "ELF has a program header with a bigger size on disk than in memory?");
        f.seek(SeekFrom::Start(program_header.p_offset as u64)).unwrap();
        assert_eq!(program_header.p_vaddr % 4, 0, "Section not aligned to a 4-byte boundary.");
        //assert_eq!(program_header.p_filesz % 4, 0, "File size not a multiple of 4.");
        //assert_eq!(program_header.p_memsz % 4, 0, "Memory size not a multiple of 4.");
        // Being lazy! Round file and memory size up to a multiple of 4. It
        // only needs to be good enough to work in the tests...
        let disk_size = if program_header.p_filesz % 4 == 0 { program_header.p_filesz }
        else { (program_header.p_filesz & !3) + 4 } as usize;
        let _mem_size = if program_header.p_memsz % 4 == 0 { program_header.p_memsz }
        else { (program_header.p_memsz & !3) + 4 } as usize;
        let mut words = vec![];
        words.reserve_exact(program_header.p_memsz as usize);
        let mut buf = [0u8; 4096];
        let mut rem = disk_size;
        while rem > 0 {
            let bytes_to_read = rem.min(buf.len());
            f.read_exact(&mut buf[..bytes_to_read]).unwrap();
            rem -= bytes_to_read;
            for word in buf[..bytes_to_read].chunks_exact(4) {
                words.push(u32::from_le_bytes([word[0], word[1], word[2], word[3]]));
            }
        }
        words.resize((program_header.p_memsz / 4) as usize, 0xdeadbeef);
        Some(LoadedChunk {
            base: program_header.p_vaddr,
            words,
        })
    }).collect();
    assert_eq!(header.e_shentsize, 40, "Section headers in ELF not 40 bytes long!");
    let mut symtab_header = None;
    let mut strtab_header = None;
    for section_number in 0 .. header.e_shnum {
        f.seek(SeekFrom::Start((header.e_shoff + header.e_shentsize as u32 * section_number as u32) as u64)).unwrap();
        let section_header = read_elf_section_header(&mut f);
        if section_header.sh_type == 2 {
            if symtab_header.is_none() {
                symtab_header = Some(section_header);
            } else { panic!("Multiple symtabs!") }
        } else if section_header.sh_type == 3 {
            if strtab_header.is_none() {
                strtab_header = Some(section_header);
            } else { /* let's skip the second one */ }
        } else { /* ignore */ }
    }
    let symtab_header = symtab_header.expect("No symtab!");
    let strtab_header = strtab_header.expect("No strtab!");
    let mut strtab = vec![0u8; strtab_header.sh_size as usize];
    assert_eq!(symtab_header.sh_entsize, 16, "Symbols are not 16 bytes?");
    f.seek(SeekFrom::Start(strtab_header.sh_offset as u64)).unwrap();
    f.read_exact(&mut strtab[..]).unwrap();
    let mut symbol_table = HashMap::new();
    f.seek(SeekFrom::Start(symtab_header.sh_offset as u64)).unwrap();
    for _ in (0 .. symtab_header.sh_size).step_by(symtab_header.sh_entsize as usize) {
        let symbol = read_elf_symbol(&mut f);
        let symbol_name = &strtab[symbol.st_name as usize .. symbol.st_name as usize + strtab[symbol.st_name as usize ..].iter().position(|x| *x==0).unwrap()];
        symbol_table.insert(symbol_name.to_vec(), symbol.st_value);
    }
    LoadedElf { sections: chunks, entry_point: header.e_entry, symbol_table }
}

struct Elfo<const A: bool, const M: bool, const C: bool> {
    ram: Vec<u32>,
    entry_point: u32,
    reserved_addr: u32,
    tohost: Option<u32>,
    symbol_table: HashMap<Vec<u8>, u32>,
}

impl<const A: bool, const M: bool, const C: bool> Elfo<A, M, C> {
    fn new(elf: LoadedElf) -> Elfo<A, M, C> {
        let mut ram = vec![0u32; 0x400000];
        for section in elf.sections.iter() {
            let start = ((section.base - 0x80000000) / 4) as usize;
            let len = section.words.len();
            ram[start..(start+len)].copy_from_slice(&section.words[..]);
        }
        Elfo { ram, entry_point: elf.entry_point, reserved_addr: !0, tohost: None, symbol_table: elf.symbol_table }
    }
    fn take_tohost(&mut self) -> Option<u32> {
        self.tohost.take()
    }
}

impl<const A: bool, const M: bool, const C: bool> ExecutionEnvironment for Elfo<A,M,C> {
    const SUPPORT_A: bool = A;
    const SUPPORT_M: bool = M;
    const SUPPORT_C: bool = C;
    fn read_word(&mut self, address: u32, _mask: u32) -> Result<u32, rrv32::MemoryAccessFailure> {
        if Self::SUPPORT_C && self.enable_c() && address % 4 == 2 {
            return Ok(self.read_half(address)? as u32 | ((self.read_half(address+2)? as u32) << 16));
        }
        if address % 4 != 0 { return Err(MemoryAccessFailure::Unaligned) }
        if address == 0xC0000000 { todo!("fromhost") }
        else if address >= 0x80000000 {
            let word_offset = ((address - 0x80000000) / 4) as usize;
            if word_offset >= self.ram.len() { return Err(MemoryAccessFailure::Fault) }
            return Ok(self.ram[word_offset])
        }
        return Err(MemoryAccessFailure::Fault)
    }
    fn write_word(&mut self, address: u32, data: u32, mask: u32) -> Result<(), rrv32::MemoryAccessFailure> {
        if address % 4 != 0 { return Err(MemoryAccessFailure::Unaligned) }
        if self.reserved_addr == address { self.reserved_addr = !0 }
        if address == 0xC0000000 {
            self.tohost = Some(data);
            return Ok(())
        }
        else if address >= 0x80000000 {
            let word_offset = ((address - 0x80000000) / 4) as usize;
            if word_offset >= self.ram.len() { return Err(MemoryAccessFailure::Fault) }
            self.ram[word_offset] &= !mask;
            self.ram[word_offset] |= data & mask;
            return Ok(())
        }
        return Err(MemoryAccessFailure::Fault)
    }
    fn load_reserved_word(&mut self, address: u32) -> Result<u32, rrv32::MemoryAccessFailure> {
        if address % 4 != 0 { return Err(MemoryAccessFailure::Unaligned) }
        let ret = self.read_word(address, !0);
        if ret.is_ok() {
            self.reserved_addr = address;
        }
        ret
    }
    fn store_reserved_word(&mut self, address: u32, data: u32) -> Result<bool, rrv32::MemoryAccessFailure> {
        if address % 4 != 0 { return Err(MemoryAccessFailure::Unaligned) }
        if self.reserved_addr != address { return Ok(false) }
        self.write_word(address, data, !0).map(|_| true)
    }
    fn csr_access<F:FloatBits>(&mut self, cpu: &mut Cpu<F>, csr_number: u32, handler: impl Fn(u32, u32) -> u32, operand: u32) -> Result<u32, ExceptionCause> {
        if F::SUPPORT_F && self.enable_f() {
            match csr_number {
                0x001 => return cpu.access_fflags(handler, operand),
                0x002 => return cpu.access_frm(handler, operand),
                0x003 => return cpu.access_fcsr(handler, operand),
                _ => (),
            }
        }
        match csr_number {
            0x300 => {
                // mstatus, no-op
                return Ok(0)
            },
            _ => (),
        }
        Err(ExceptionCause::IllegalInstruction)
    }
}

fn run_inner<F: FloatBits, const A: bool, const M: bool, const C: bool>(signature_path: &str, mut elfo: Elfo<A,M,C>) {
    let mut cpu = rrv32::Cpu::<F>::new();
    cpu.put_pc(elfo.entry_point);
    loop {
        match cpu.step(&mut elfo) {
            Ok(_) => (),
            Err(x) => {
                panic!("Error {x:?}, signature_path={signature_path:?}"); 
            },
        }
        match elfo.take_tohost() {
            Some(x) if x & 1 == 1 => {
                if x == 1 { break } // peacefully stop executing
                else {
                    panic!("Test requested an error exit!");
                }
            },
            None => (),
            Some(tohost) => panic!("Unknown tohost value: {tohost}/0x{tohost:X}"),
        }
    }
    let sig_begin = *elfo.symbol_table.get(b"rvtest_sig_begin" as &[u8]).expect("missing rvtest_sig_begin symbol");
    let sig_end = *elfo.symbol_table.get(b"rvtest_sig_end" as &[u8]).expect("missing rvtest_sig_end symbol");
    assert!(sig_end >= sig_begin);
    assert!(sig_begin % 4 == 0);
    assert!(sig_end % 4 == 0);
    let mut f = File::create(signature_path).unwrap();
    for sigaddr in (sig_begin .. sig_end).step_by(4) {
        write!(f, "{:08x}\n", elfo.read_word(sigaddr, !0).unwrap()).unwrap();
    }
}

fn run_outer<F: FloatBits>(signature_path: &str, support_a: bool, support_m: bool, support_c: bool, elf: LoadedElf) {
    match (support_a, support_m, support_c) {
        (false, false, false) => run_inner::<F, false, false, false>(signature_path, Elfo::new(elf)),
        (true, false, false) => run_inner::<F, true, false, false>(signature_path, Elfo::new(elf)),
        (false, true, false) => run_inner::<F, false, true, false>(signature_path, Elfo::new(elf)),
        (true, true, false) => run_inner::<F, true, true, false>(signature_path, Elfo::new(elf)),
        (false, false, true) => run_inner::<F, false, false, true>(signature_path, Elfo::new(elf)),
        (true, false, true) => run_inner::<F, true, false, true>(signature_path, Elfo::new(elf)),
        (false, true, true) => run_inner::<F, false, true, true>(signature_path, Elfo::new(elf)),
        (true, true, true) => run_inner::<F, true, true, true>(signature_path, Elfo::new(elf)),
    }
}

fn main() {
    let (isa, signature_path, exe_path) = parse_args();
    const ISA_PREDICATES: &[fn(&str) -> Option<String>] = &[
        |isa| if !isa.starts_with("rv32") {
            Some("ISA must start with 'rv32'".to_string())
        } else { None },
        |isa| if !isa[4..].contains("i") {
            Some("'i' must be present in ISA".to_string())
        } else { None },
        |isa| {
            for el in isa[4..].chars() {
                if !"imafdqc".contains(el) {
                    return Some(format!("Unknown ISA extension {el:?}"))
                }
            }
            return None
        },
        |isa| if isa[4..].contains("q") && !isa[4..].contains("d") {
            Some("'d' must be present in ISA if 'q' is".to_string())
        } else { None },
        |isa| if isa[4..].contains("d") && !isa[4..].contains("f") {
            Some("'f' must be present in ISA if 'd' is".to_string())
        } else { None },
    ];
    for predicate in ISA_PREDICATES.iter() {
        if let Some(error) = predicate(isa.as_str()) {
            println!("{}", error);
            std::process::exit(1);
        }
    }
    let float_isa =
        if isa[4..].contains("q") { FloatISA::Q }
        else if isa[4..].contains("d") { FloatISA::D }
        else if isa[4..].contains("f") { FloatISA::F }
        else { FloatISA::None };
    let support_a = isa[4..].contains("a");
    let support_m = isa[4..].contains("m");
    let support_c = isa[4..].contains("c");
    let elf = load_elf(&exe_path);
    match float_isa {
        FloatISA::None => run_outer::<()>(&signature_path, support_a, support_m, support_c, elf),
        FloatISA::F => run_outer::<u32>(&signature_path, support_a, support_m, support_c, elf),
        FloatISA::D => run_outer::<u64>(&signature_path, support_a, support_m, support_c, elf),
        FloatISA::Q => run_outer::<u128>(&signature_path, support_a, support_m, support_c, elf),
    }
}
