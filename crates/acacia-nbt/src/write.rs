use bytes::{BufMut, BytesMut};

use crate::{Flavor, Nbt, Value};

/// Writes a root tag. The tree must be well formed (README.md, "Writing").
pub fn write<F: Flavor>(w: &mut BytesMut, nbt: &Nbt) {
    let tag = nbt.value.tag();
    w.put_u8(tag);
    if tag != 0 {
        write_str::<F>(w, &nbt.name);
        write_payload::<F>(w, &nbt.value);
    }
}

fn write_str<F: Flavor>(w: &mut BytesMut, s: &str) {
    // A longer string would wrap the length prefix and corrupt everything after it.
    let s = &s[..s.floor_char_boundary(F::MAX_STR)];
    F::write_str_len(w, s.len());
    w.put_slice(s.as_bytes());
}

fn write_payload<F: Flavor>(w: &mut BytesMut, v: &Value) {
    match v {
        Value::End => {}
        Value::Byte(x) => w.put_i8(*x),
        Value::Short(x) => w.put_i16_le(*x),
        Value::Int(x) => F::write_int(w, *x),
        Value::Long(x) => F::write_long(w, *x),
        Value::Float(x) => w.put_f32_le(*x),
        Value::Double(x) => w.put_f64_le(*x),
        Value::ByteArray(b) => {
            F::write_int(w, b.len() as i32);
            w.put_slice(b);
        }
        Value::String(s) => write_str::<F>(w, s),
        Value::List(l) => {
            debug_assert!(l.items.iter().all(|i| i.tag() == l.tag && l.tag != 0), "list items must match its tag");
            w.put_u8(l.tag);
            F::write_int(w, l.items.len() as i32);
            for item in &l.items {
                write_payload::<F>(w, item);
            }
        }
        Value::Compound(entries) => {
            for (name, value) in entries {
                debug_assert!(value.tag() != 0, "an End entry would end the compound early");
                w.put_u8(value.tag());
                write_str::<F>(w, name);
                write_payload::<F>(w, value);
            }
            w.put_u8(0);
        }
        Value::IntArray(a) => {
            F::write_int(w, a.len() as i32);
            a.iter().for_each(|x| F::write_int(w, *x));
        }
        Value::LongArray(a) => {
            F::write_int(w, a.len() as i32);
            a.iter().for_each(|x| F::write_long(w, *x));
        }
    }
}
