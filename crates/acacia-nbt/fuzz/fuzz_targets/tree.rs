//! Well-formed trees through both flavours' `write`, then `read` and `skip`.
#![no_main]

use acacia_nbt::{LittleEndian, List, Nbt, Network, Str, Value};
use arbitrary::{Result, Unstructured};
use libfuzzer_sys::fuzz_target;

#[allow(dead_code)]
#[path = "../../tests/common/props.rs"]
mod props;

const MAX_DEPTH: usize = 32;
const MAX_CHILDREN: usize = 8;

/// Containers only above `MAX_DEPTH`, so the tree ends.
fn pick_tag(u: &mut Unstructured, depth: usize) -> Result<u8> {
    u.int_in_range(1..=if depth < MAX_DEPTH { 12 } else { 8 })
}

fn string(u: &mut Unstructured) -> Result<Str> {
    Ok(u.arbitrary::<String>()?.into())
}

fn value(u: &mut Unstructured, tag: u8, depth: usize) -> Result<Value> {
    Ok(match tag {
        1 => Value::Byte(u.arbitrary()?),
        2 => Value::Short(u.arbitrary()?),
        3 => Value::Int(u.arbitrary()?),
        4 => Value::Long(u.arbitrary()?),
        5 => Value::Float(u.arbitrary()?),
        6 => Value::Double(u.arbitrary()?),
        7 => Value::ByteArray(u.arbitrary()?),
        8 => Value::String(string(u)?),
        9 => {
            let tag = pick_tag(u,depth + 1)?;
            let n = u.int_in_range(0..=MAX_CHILDREN)?;
            let items = (0..n).map(|_| value(u, tag, depth + 1)).collect::<Result<_>>()?;
            Value::List(List { tag, items })
        }
        10 => {
            let n = u.int_in_range(0..=MAX_CHILDREN)?;
            let entries = (0..n)
                .map(|_| {
                    let tag = pick_tag(u,depth + 1)?;
                    Ok((string(u)?, value(u, tag, depth + 1)?))
                })
                .collect::<Result<_>>()?;
            Value::Compound(entries)
        }
        11 => Value::IntArray(u.arbitrary()?),
        _ => Value::LongArray(u.arbitrary()?),
    })
}

fn root(u: &mut Unstructured) -> Result<Nbt> {
    let tag = pick_tag(u,0)?;
    Ok(Nbt { name: u.arbitrary()?, value: value(u, tag, 0)? })
}

fuzz_target!(|data: &[u8]| {
    let Ok(nbt) = root(&mut Unstructured::new(data)) else {
        return;
    };
    props::tree::<Network>(&nbt);
    props::tree::<LittleEndian>(&nbt);
});
