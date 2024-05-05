#![doc=include_str!("../README.md")]

mod cpu;
pub use cpu::*;
mod execution;
pub use execution::*;

/// The value that should be returned when the `mvendorid` CSR is read.
///
/// This is defined as 0, indicating that we are an open source
/// non-commercial implementation of the architecture.
pub const VENDOR_ID: u32 = 0;
/// The value that should be returned when the `marchid` CSR is read.
///
/// An ID of 45 was assigned by the RISC-V Foundation for the rrv32 core.
pub const ARCH_ID: u32 = 45;
/// The value that should be returned when the `mimpid` CSR is read.
///
/// This value encodes in each byte (in order of decreasing significance:
///
/// - Extra flags. (Currently 0. If we add JIT this will be where it can be
///   detected.)
/// - Major version.
/// - Minor version.
/// - Patch version.
///
/// So, for example, the value 0x001A0530 describes version 26.5.48
/// (0x1A.0x05.0x30) of rrv32.
///
/// This value will change whenever the version number of the `rrv32` crate
/// does. Some applications will want to store the value of this constant
/// at power-on, and when they deserialize a "running" CPU, continue returning
/// the value that was stored at power-on instead of letting the emulated
/// machine witness `mimpid` changing. (Specifically, applications
/// where determinism is absolutely essential, such as during replay playback
/// or lockstep replication.)
pub const IMPLEMENTATION_ID: u32 = u32::from_be_bytes([
    0,
    parse_const_u8(env!("CARGO_PKG_VERSION_MAJOR").as_bytes()),
    parse_const_u8(env!("CARGO_PKG_VERSION_MINOR").as_bytes()),
    parse_const_u8(env!("CARGO_PKG_VERSION_PATCH").as_bytes()),
]);

const fn parse_const_u8(x: &'static [u8]) -> u8 {
    match x.len() {
        0 => panic!("a version environment variable was empty!"),
        1 => x[0] - b'0',
        2 => (x[0] - b'0') * 10 + (x[1] - b'0'),
        3 => (x[0] - b'0') * 100 + (x[1] - b'0') * 10 + (x[2] - b'0'),
        _ => panic!("a version environment variable was absurdly long!"),
    }
}
