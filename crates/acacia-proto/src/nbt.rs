//! NBT (`acacia-nbt`) as packets carry it: decode errors as [`DecodeError`], strict-mode reporting
//! and minecraft-data's `nbtLoop` framing.

use bytes::{BufMut, BytesMut};

pub use acacia_nbt::{Flavor, LittleEndian, List, Nbt, Network, Raw, Str, Value, write};

use crate::codec::Result;
use crate::strict::{Leniency, note};

/// A packet-level tag: only its end is found here (docs/proto.md, "Lazy NBT").
pub fn read_raw<F: Flavor>(r: &mut &[u8]) -> Result<Raw<F>> {
    let (raw, lossy) = Raw::read_lossy(r)?;
    if lossy {
        note(Leniency::Utf8);
    }
    Ok(raw)
}

pub fn read<F: Flavor>(r: &mut &[u8]) -> Result<Nbt> {
    let (nbt, lossy) = acacia_nbt::read_lossy::<F>(r)?;
    if lossy {
        note(Leniency::Utf8);
    }
    Ok(nbt)
}

pub fn skip<F: Flavor>(r: &mut &[u8]) -> Result<()> {
    Ok(acacia_nbt::skip::<F>(r)?)
}

/// Reads root tags until a `0` terminator byte or end of input (minecraft-data `nbtLoop`).
pub fn read_loop(r: &mut &[u8]) -> Result<Vec<Nbt>> {
    let mut out = Vec::new();
    while let Some(&b) = r.first() {
        if b == 0 {
            *r = &r[1..];
            break;
        }
        out.push(read::<Network>(r)?);
    }
    Ok(out)
}

pub fn write_loop(w: &mut BytesMut, v: &[Nbt]) {
    v.iter().for_each(|n| write::<Network>(w, n));
    w.put_u8(0);
}
