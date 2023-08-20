use std::{
    io::{Read, BufRead},
};

use anyhow::{anyhow, Context};

pub fn initial_program_load<R: BufRead>(buf: &mut [u32], mut reader: R) -> anyhow::Result<()> {
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
