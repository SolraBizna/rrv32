RISC-V is a powerful, elegant CPU instruction set architecture that is also an open standard. `rrv32` is a crate providing a software implementation of this architecture. You can use it to create a 32-bit RISC-V emulator. It is intended to form the core of the (yet unreleased) `tatsui` crate, which will provide a batteries-included drop-in computer system for use in computer games and education.

`rrv32` supports all of RV32GCQ_Zicsr_Zifence. To unpack that one piece at a time:

- RV32: 32-bit version of the RISC-V ISA. 32-bit address space, 32-bit general purpose registers, 32-bit ALU, 32-bit data bus.
- G: Shorthand for IMAFD. The bare minimum subset of the architecture considered good enough for a "full computer" (something you would expect to be able to run Linux on, for example).
  - I: Version of the basic instruction set that has 31 general purpose registers. (As opposed to E, which has only 15.)
  - M: Multiplication and division instructions.
  - A: Atomic memory operations.
  - F: 32-bit floating point instructions.
  - D: 64-bit floating point instructions.
- C: Support for compressing certain common instructions into 16 bits, significantly reducing program size. (As opposed to every instruction taking up an entire 32 bits.)
- Q: 128-bit floating point instructions.
- Zicsr: Instructions for reading and writing hardware "control and status registers".
- Zifence: An instruction that informs the CPU's instruction cache that some instructions stored in the instruction cache may no longer be valid (because the memory underlying them has changed).

`rrv32` does not implement anything outside of the purview of these standards. In particular, it only implements components of the RISC-V *unprivileged* specification. While it does not contain any implementation of the RISC-V *privileged* specification, it is carefully designed such that your `ExecutionEnvironment` can provide a full implementation of the privileged specification—or any other paradigm you care to imagine that fills the same role.

In keeping with its intended role as the core of a simulated computer in a video game, `rrv32` provides hooks for accounting for emulated CPU time. You can assign costs to each operation performed by the emulated CPU, so that the emulated computer can't starve the rest of the game of CPU time by performing a tight loop of expensive operations, for example. These hooks are *not* granular enough to perform cycle-accurate emulation of any but the most unsophisticated RISC-V hardware; they are, instead, designed to add as little overhead to the emulation as possible.

For more information on RISC-V, including up-to-date versions of the privileged and unprivileged specifications, see the [RISC-V International® website](https://riscv.org/technical/specifications/).

# Why?

When teaching, I find it useful to have my own implementation of a given ISA handy. It can spot check my students' implementation, helping me clarify ambiguous or confusing points of the relevant standards.

I also like it when games have real computers in them. Games like _Stationeers_, _Duskers_, _Hacknet_ have simulated computers in them, but they always come with their own limitations and concessions. Lua or JavaScript are often used to bridge this gap, but the experience of writing a Lua script against a game engine is *very* different from that of writing code (even Lua code!) that runs on a real microcontroller.

A permissibly-licensed, freely-available library that provides a self-contained "real" computer system significantly lowers the barrier to entry for including "real" computer systems in games. My W65C02S emulators technically already provide this, but, shockingly, almost nobody wants to program 6502 assembly in games. With a modular RISC-V simulator, programming in C or even a language like Rust becomes possible.

This crate doesn't provide its own execution environment, assembler, linker, or compiler. One thing at a time. :)

# How?

Implement the `ExecutionEnvironment` trait. At minimum, this requires you to define your memory space and implement the following four operations:

- `read_word`: Read a 32-bit word from memory.
- `write_word`: Write a 32-bit word to memory, possibly leaving some bytes unaffected (depending on `mask`). If the write overlaps with the reserved word, break the reservation.
- `load_reserved_word`: Read a 32-bit word from memory, and mark the address of that word as reserved.
- `store_reserved_word`: If the target address has been reserved with `load_reserved_word`, and that reservation has not been clobbered by to a later `write_word` to that address, write a word to that address.

Now all you must do is:

- Instantiate `Rv32I`, `Rv32IF`, `Rv32G`, or `Rv32GQ`, depending on whether you want to support no floats, 32-bit floats, 64-bit floats, or 128-bit floats, respectively. (When in doubt, use `Rv32G`.)
- Set the PC using `cpu.put_pc`.
- Call `cpu.step` a bunch of times with a mutable reference to your `ExecutionEnvironment`.

Oh, and don't forget to have some kind of program loaded in the memory space created by your `ExecutionEnvironment`, or nothing interesting will happen. :)

See `src/bin/ttybox.rs` for a very simple example. It emulates a particular terminal-based system which I often have my students implement in a logic simulator. (This is why it ingests programs in the form of Logisim memory dumps.)

"Defining your memory space" is actually a huge amount of work and pain. If you want a much more batteries-included solution... when the `tatsui` crate is complete, I will link it here.

# Feature Flags

By default, the `C` and `float` features are enabled and the `serde` feature is disabled.

## `C`

Compiles in code relating to the `C` (compressed instructions) extension. You can disable `C` support without removing this feature flag, removing it just saves a little compile time in the case where you *know* you will never want the `C` extension.

(The `C` extension has nothing at all to do with the C programming language.)

## `float`

Compiles in code and dependencies relating to the floating point extensions (`FDQ`). You can disable float support without removing this feature flag, removing it just saves some compile time and avoids pulling in float-related dependencies.

## `serde`

Implements [Serde](https://serde.rs)'s `Serialize` and `Deserialize` traits for `Cpu`. This is the only practical way to save and restore the emulated CPU's entire state. This feature flag is disabled by default because `serde` is a relatively hefty dependency; without it `rrv32` is quite lean.

We took care to make the serialized CPU as compact as possible in any particular representation format.

# RISC-V Extensions

All extensions listed below can be turned on or off by your `ExecutionEnvironment` implementation. Some can also be disabled by compiling `rrv32` without certain feature flags.

## M (multiplication and division)

Full support.

## A (atomic memory operations)

Full support. The burden of implementing reserved load/store is on your `ExecutionEnvironment` (but it's not complicated). Not thoroughly tested. Bug reports welcome.

## F/D/Q (floating point)

(Requires the `float` feature flag, enabled by default.)

F/D/Q support depends on the specialization of `Cpu`.

- `pub type Rv32I = Cpu<()>;`: Default. No floating point support. CPU state is 128 bytes.
- `pub type Rv32IF = Cpu<u32>;`: F (single precision) support only. CPU state is 260 bytes.
- `pub type Rv32G = Cpu<u64>;`: D (double precision) and F support. CPU state is 388 or 392 bytes depending on your architecture. (When in doubt, use this one.)
- `pub type Rv32GQ = Cpu<u128>;`: Q (quad precision) and D and F support. CPU state is 644, 648, or 656 bytes depending on your architecture.

(If future versions of this crate support RV32E or RV64, manually specializing `Cpu` yourself may be deprecated. The type aliases given above will always work.)

Double- and quad-precision floating point loads and stores are NOT ATOMIC. This is allowed by the standard, at least for 32-bit cores. They also only require 4-byte alignment. This simulator doesn't provide a way to fault on non-8-byte-aligned double loads and stores. If you need that behavior for some reason, sorry!

All rounding modes and floating point exception flags are fully handled. We use `rustc_apfloat` to do most of the heavy lifting here. We avoid native floating point support because that would expose us to the subtle differences in floating point implementations for different architectures.

### Accuracy

I believe there are a few edge cases involving "barely infinities" that this core gets slightly wrong. The official simulator gets those cases wronger (it seems). Outside of these cases, the floating point accuracy here is solid to the last ulp.

### `SQRT`

`SQRT` is a special case. We use `ieee-apsqrt` to perform it, which means we have a choice between "fast" and "accurate" `SQRT.F` and `SQRT.D`, but only "fast" `SQRT.Q`. The fast versions get the last one or two ULPs wrong for some inputs. The execution environment can choose whether the fast or accurate version is used for each square root instruction. If accurate `SQRT.Q` is requested, `SQRT.Q` becomes an illegal instruction, because accurate `SQRT.Q` is not implemented yet! The current version defaults `SQRT.F` and `SQRT.D` to accurate, and `SQRT.Q` to fast. The latter default will change if accurate `SQRT.Q` becomes implemented.

`ieee-apsqrt` uses Newton-Raphson to perform square roots. It slightly more than doubles the number of significand bits when calculating the "accurate" version. Bear this in mind if you're game-balancing floating point operations.

## C (compressed instructions)

(Requires the `C` feature flag, enabled by default.)

Fully supported.

## Zhf (half-precision floats)

I haven't implemented this extension, but I will if anyone wants it.

## Zicsr (control and status register instructions)

Fully implemented. If you need any CSRs other than the floating point ones, your `ExecutionEnvironment` is in charge of implementing the individual registers. `rrv32` implements every `CSR*` instruction, and provides an easy-to-implement interface for defining new CSRs in your `ExecutionEnvironment` without having to worry about which variants of which `CSR*` instructions should be read- or write-only or which bit operation is supposed to be used etc.

## Zifence (`IFENCE` instruction)

Implemented as a no-op.

# Compliance

`rrv32` passes all relevant RISC-V compliance tests. Notable exceptions:

- Several `F` tests. I believe all currently-failing tests to be bugs in the reference simulator. (In a few of those cases, my simulator also gives wrong answers, but they're *different* wrong answers.)
- `D`: The compliance tests infinite loop and nuke my hard drive, so I can't run them.
- `A`, `Q`: No official compliance tests at the time of this writing.
- Some tests assume parts of the privileged ISA, and have to be manually pruned.

If you want to run RISCOF yourself against `rrv32`, the `riscof-dut` binary in this repository will be of use.

# Performance

Running the [`embench-iot`](https://github.com/embench/embench-iot/) benchmark suite against the first feature-complete version of rrv32:

|        CPU        | Host speed |  Emu speed  | Worst ratio |
| ----------------- | ---------- | ----------- | ----------- |
| AMD Ryzen 5 5600X |     4.2GHz | 81-250 MIPS |          52 |
|   Apple M1 P-core |     3.2GHz | 70-276 MIPS |          46 |
|   Apple M1 E-core |     1.3GHz |  23-75 MIPS |          57 |

Putting a single ~1MIPS simulated RISC-V core in a singlethreaded game loop should be achievable without unacceptable performance loss. If you want more cores or higher speeds, multithreading will help a great deal. Bear in mind that, depending on what you're doing with them, computers are still useful down to the single digit kHz range!

Performance could be greatly improved with JIT, but I have already gone too far down the rabbit hole... :)

# Legalese

`rrv32` is copyright 2023 and 2024, Solra Bizna, and licensed under either of:

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or
   <http://www.apache.org/licenses/LICENSE-2.0>)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the `rrv32` crate by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
