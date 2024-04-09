use std::mem::size_of;

use serde::{
    de::Unexpected, ser::SerializeStruct, Deserialize, Serialize, Serializer,
};

use super::{Cpu, FloatBits, FloatStatusTrait};

impl<F: FloatBits> Serialize for Cpu<F> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Output (is_big_endian, bytes)
        let has_float = size_of::<F>() != 0;
        let mut tuple = serializer.serialize_struct(
            "SerializedCpu",
            if has_float { 5 } else { 2 },
        )?;
        let mut buf = [0; 512];
        let mut out_n = 0;
        for in_n in 0..32 {
            let b = self.registers[in_n].to_be_bytes();
            buf[out_n] = b[0];
            buf[out_n + 1] = b[1];
            buf[out_n + 2] = b[2];
            buf[out_n + 3] = b[3];
            out_n += 4;
        }
        tuple.serialize_field("registers", &buf[..out_n])?;
        tuple.serialize_field("float_bytes", &(size_of::<F>() as u8))?;
        if has_float {
            tuple.serialize_field("fcsr", &self.read_fcsr())?;
            tuple.serialize_field("fstatus", &self.fstatus.get_bits())?;
            out_n = 0;
            for in_n in 0..32 {
                let b = self.float_registers[in_n].to_bytes();
                buf[out_n..out_n + F::BYTES_PER_FLOAT]
                    .copy_from_slice(&b[..F::BYTES_PER_FLOAT]);
            }
            tuple.serialize_field("float_registers", &buf[..out_n])?;
        }
        tuple.end()
    }
}

// Writing this deserializer by hand would have been an enormous PITA,
// and deserializing is much less performance-critical than serializing,
// so we can use a derived deserializer and an intermediate struct to make
// it easier.
//
// Except that there are actually two intermediate structs, one which has float
// support and one which lacks it. And some formats might predicate on the
// actual name of the struct. Thus... this.

mod nofloat {
    use super::*;
    #[derive(Deserialize)]
    pub struct SerializedCpu {
        pub registers: Vec<u8>,
        pub float_bytes: u8,
    }
}

mod yesfloat {
    use super::*;
    #[derive(Deserialize)]
    pub struct SerializedCpu {
        pub registers: Vec<u8>,
        pub float_bytes: u8,
        pub fcsr: u8,
        pub fstatus: u8,
        pub float_registers: Vec<u8>,
    }
}

impl<'de, F: FloatBits> Deserialize<'de> for Cpu<F> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;
        let mut ret: Self = Self::new();
        let (registers, float_bytes, fcsr, fstatus, float_registers);
        if std::mem::size_of::<F>() == 0 {
            let intermediate =
                nofloat::SerializedCpu::deserialize(deserializer)?;
            registers = intermediate.registers;
            float_bytes = intermediate.float_bytes;
            fcsr = 0;
            fstatus = 0;
            float_registers = vec![];
        } else {
            let intermediate =
                yesfloat::SerializedCpu::deserialize(deserializer)?;
            registers = intermediate.registers;
            float_bytes = intermediate.float_bytes;
            fcsr = intermediate.fcsr;
            fstatus = intermediate.fstatus;
            float_registers = intermediate.float_registers;
        }
        if float_bytes as usize != std::mem::size_of::<F>() {
            return Err(Error::invalid_value(
                Unexpected::Unsigned(float_bytes as u64),
                &"expected a float_bytes value that matches our configuration",
            ));
        }
        if registers.len() != 256 {
            return Err(Error::invalid_length(
                registers.len(),
                &"expected 256 bytes of register data",
            ));
        }
        if float_registers.len() != std::mem::size_of::<F>() * 32 {
            return Err(Error::invalid_length(
                registers.len(),
                &format!(
                    "expected {} bytes of float register data",
                    std::mem::size_of::<F>() * 32
                )
                .as_str(),
            ));
        }
        for (i, bytes) in registers.chunks_exact(4).enumerate() {
            ret.registers[i] = u32::from_be_bytes(bytes.try_into().unwrap());
        }
        if std::mem::size_of::<F>() != 0 {
            ret.write_fcsr(fcsr as u32);
            ret.fstatus.set_bits(fstatus);
            for (i, bytes) in float_registers
                .chunks_exact(std::mem::size_of::<F>())
                .enumerate()
            {
                ret.float_registers[i] = F::from_bytes(bytes);
            }
        }
        Ok(ret)
    }
}
