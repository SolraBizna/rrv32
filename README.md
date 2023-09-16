This is an RV32GCQ simulation crate. It implements all of the unprivileged behavior of the 32-bit RISC-V I, M, A, F, D, C, Q, Zifence, and Zicsr standards, and provides enough plumbing to implement all of the privileged standard. It has support for roughly accounting for and limiting execution time, which is useful for profiling and/or game design purposes.

# Why

At least one of my students is working on a RISC-V implementation. When teaching, I find it useful to have my own implementation of a given ISA handy. It can spot check my student's implementation, and clarify ambiguous [or confusing](https://cdn.discordapp.com/attachments/340907123510607873/1151757802554019910/Screenshot_2023-09-13_at_11.53.44_PM.png) points of the relevant standards.

I also like it when games have real computers in them. Games like _Stationeers_, _Duskers_, _Hacknet_ have computers in them, but they always come with their own limitations and concessions. Lua or JavaScript are often used to solve this problem, but that brings in a whole different set of problems. If there were a permissibly-licensed, freely-available library that provided a self-contained "real" computer system, the barrier to entry is significantly lowered for including "real" computer systems in games. My W65C02S emulators technically already provide this, but, shockingly, nobody wants to program 6502 assembly in games. With a modular RISC-V simulator, programming in C or even a language like Rust becomes possible.

This crate doesn't provide its own execution environment, assembler, linker, or compiler. One thing at a time. :)

# Extensions

All extensions listed below can be turned on or off by your `ExecutionEnvironment` implementation.

## I (base RV32)

I is always enabled, i.e. there is no provision to simulate RV32E.

## M (multiplication and division)

Full support.

## A (atomic memory operations)

Full support. The burden of implementing reserved load/store is on your `ExecutionEnvironment` (but it's not complicated). Not thoroughly tested. Bug reports welcome.

## F/D/Q (floating point)

F/D/Q support depends on the specialization of `Cpu`.

- `Cpu<()>`: Default. No floating point support. CPU state is 128 bytes. 
- `Cpu<f32>`: F (single precision) support only. CPU state is 260 bytes.
- `Cpu<f64>`: D (double precision) and F support. CPU state is 388 or 392 bytes depending on your architecture.
- `Cpu<f128>`: Q (quad precision) and D and F support. CPU state is 644, 648, or 656 bytes depending on your architecture.

G requires D, so to actually simulate RV32G, make sure you specify `<f64>` on your `Cpu`.

Double- and quad-precision floating point loads and stores are NOT ATOMIC. This is allowed by the standard, at least for 32-bit cores. They also only require 4-byte alignment. This simulator doesn't provide a way to fault on non-8-byte-aligned double loads and stores. If you need that behavior for some reason, sorry!

All rounding modes and floating point exception flags should be fully handled. We use `rustc_apfloat` to do most of the heavy lifting here.

### Accuracy

I believe there are a few edge cases involving "barely infinities" that this core gets slightly wrong. The official simulator gets those cases wronger (it seems). Outside of these cases, the floating point accuracy here is solid to the last ulp.

### `SQRT`

`SQRT` is a special case. We use `ieee-apsqrt` to perform it, which means we have a choice between "fast" and "accurate" `SQRT.F` and `SQRT.D`, but only "fast" `SQRT.Q`. The fast versions get the last one or two ULPs wrong for some inputs. The execution environment can choose whether the fast or accurate version is used for each square root instruction. If accurate `SQRT.Q` is requested, `SQRT.Q` becomes an illegal instruction, because accurate `SQRT.Q` is not implemented yet! The current version defaults `SQRT.F` and `SQRT.D` to accurate, and `SQRT.Q` to fast. The latter default will change if accurate `SQRT.Q` becomes implemented.

`ieee-apsqrt` uses Newton-Raphson to perform square roots. It slightly more than doubles the number of significand bits when calculating the "accurate" version. Bear this in mind if you're game-balancing floating point operations.

## C (compressed instructions)

Fully supported.

## Zhf (half-precision floats)

I haven't implemented this extension, but I will if anyone wants it.

## Zicsr (control and status register instructions)

Fully implemented. If you need any CSRs other than the floating point ones, your `ExecutionEnvironment` is in charge of implementing the individual registers, but the different `CSR*` instructions are implemented for you.

## Zifence (`IFENCE` instruction)

Implemented as a no-op.

# Compliance

`rrv32` passes all relevant RISC-V compliance tests. Notable exceptions:

- Several `F` tests. I believe all currently-failing tests to be bugs in the reference simulator. (A few of them, I also suspect are *different* bugs in *my* simulator.)
- `D`: The compliance tests infinite loop and nuke my hard drive, so I can't run them.
- `A`, `Q`: No official compliance tests.
- Some tests that assume parts of the privileged ISA have to be manually pruned.

# Performance

The performance of `rrv32` hasn't yet been characterized much. It's faster than the SAIL RISC-V simulator, but that's not exactly a high bar; performance is, for good reasons, low on the SAIL simulator's priority list.

# Legalese

`rrv32` is copyright 2023, Solra Bizna, and licensed under either of:

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or
   <http://www.apache.org/licenses/LICENSE-2.0>)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the `rrv32` crate by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
