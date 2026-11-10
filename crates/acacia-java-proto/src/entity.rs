//! Entity movement encodings: `VecDelta` and `PositionPath`.

use crate::{Error, VarInt, Wire, read_varint};

const DELTA_SCALE: f64 = 4096.0;

/// `VecDeltaCodec.decode`: a delta of zero on an axis keeps the base exactly.
pub fn apply_delta(base: [f64; 3], delta: [i16; 3]) -> [f64; 3] {
    std::array::from_fn(|i| if delta[i] == 0 { base[i] } else { ((base[i] * DELTA_SCALE).round() + f64::from(delta[i])) / DELTA_SCALE })
}

/// `MoveEntity.Pos`/`PosRot` after the entity id: on-ground flag, then the final position from `base`.
pub fn read_move(input: &mut &[u8], base: [f64; 3]) -> Result<(bool, [f64; 3]), Error> {
    let properties = read_varint(input)?;
    let steps = (properties as u32) >> 1;
    let mut position = base;
    if steps == 0 {
        position = apply_delta(base, read_delta(input)?);
    }
    for _ in 0..steps {
        VarInt::read(input)?;
        position = apply_delta(position, read_delta(input)?);
    }
    Ok((properties & 1 != 0, position))
}

fn read_delta(input: &mut &[u8]) -> Result<[i16; 3], Error> {
    Ok([i16::read(input)?, i16::read(input)?, i16::read(input)?])
}

/// `PositionPath`'s final position.
pub fn read_path_end(input: &mut &[u8]) -> Result<[f64; 3], Error> {
    let vec3 = |input: &mut &[u8]| -> Result<[f64; 3], Error> { Ok([f64::read(input)?, f64::read(input)?, f64::read(input)?]) };
    match read_varint(input)? {
        0 => vec3(input),
        _ => {
            let mut end = [0.0; 3];
            for _ in 0..read_varint(input)? {
                end = vec3(input)?;
                VarInt::read(input)?;
            }
            Ok(end)
        }
    }
}

/// `LpVec3.read`.
pub fn read_lp_vec3(input: &mut &[u8]) -> Result<[f64; 3], Error> {
    let lowest = u8::read(input)?;
    if lowest == 0 {
        return Ok([0.0; 3]);
    }
    let middle = u8::read(input)?;
    let highest = i32::read(input)? as u32 as u64;
    let buffer = highest << 16 | u64::from(middle) << 8 | u64::from(lowest);
    let mut scale = u64::from(lowest & 3);
    if lowest & 4 != 0 {
        scale |= u64::from(read_varint(input)? as u32) << 2;
    }
    let unpack = |bits: u64| (bits & 32767).min(32766) as f64 * 2.0 / 32766.0 - 1.0;
    Ok([3, 18, 33].map(|shift| unpack(buffer >> shift) * scale as f64))
}

/// `LpVec3.write`.
pub fn write_lp_vec3(v: [f64; 3], out: &mut Vec<u8>) {
    let v = v.map(|c| if c.is_nan() { 0.0 } else { c.clamp(-LP_ABS_MAX, LP_ABS_MAX) });
    let longest = v.iter().fold(0.0_f64, |m, c| m.max(c.abs()));
    if longest < LP_ABS_MIN {
        out.push(0);
        return;
    }
    let scale = longest.ceil() as u64;
    let partial = scale & 3 != scale;
    let markers = if partial { scale & 3 | 4 } else { scale };
    let pack = |c: f64| ((c / scale as f64 * 0.5 + 0.5) * 32766.0).round() as u64;
    let buffer = markers | pack(v[0]) << 3 | pack(v[1]) << 18 | pack(v[2]) << 33;
    out.push(buffer as u8);
    out.push((buffer >> 8) as u8);
    ((buffer >> 16) as u32 as i32).write(out);
    if partial {
        VarInt((scale >> 2) as i32).write(out);
    }
}

const LP_ABS_MAX: f64 = 1.7179869183E10;
const LP_ABS_MIN: f64 = 3.051944088384301E-5;

/// A vector in `LpVec3`'s encoding, as a packet field.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LpVec3(pub [f64; 3]);

impl Wire for LpVec3 {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        read_lp_vec3(input).map(LpVec3)
    }

    fn write(&self, out: &mut Vec<u8>) {
        write_lp_vec3(self.0, out);
    }
}

/// An `LpVec3` nothing reads, such as an entity's spawn velocity.
pub fn skip_lp_vec3(input: &mut &[u8]) -> Result<(), Error> {
    read_lp_vec3(input).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deltas_go_through_the_4096_grid_unless_zero() {
        assert_eq!(apply_delta([0.1, 2.0, 3.0], [0, 4096, -2048]), [0.1, 3.0, 2.5]);
        assert_eq!(apply_delta([0.1, 0.0, 0.0], [1, 0, 0])[0], (410.0 + 1.0) / 4096.0);
    }

    #[test]
    fn stepped_moves_end_at_the_last_step() {
        // On ground, two steps of 1.0 and 0.5 along x.
        let mut buf = vec![(2 << 1) | 1];
        for dx in [4096i16, 2048] {
            buf.push(3);
            dx.write(&mut buf);
            0i16.write(&mut buf);
            0i16.write(&mut buf);
        }
        assert_eq!(read_move(&mut &buf[..], [1.0, 2.0, 3.0]), Ok((true, [2.5, 2.0, 3.0])));
    }

    #[test]
    fn lp_vec3_skips_by_marker() {
        let mut zero: &[u8] = &[0, 9];
        skip_lp_vec3(&mut zero).unwrap();
        assert_eq!(zero, [9]);
        let mut partial: &[u8] = &[4, 0, 0, 0, 0, 0, 1, 9];
        skip_lp_vec3(&mut partial).unwrap();
        assert_eq!(partial, [9]);
    }

    #[test]
    fn lp_vec3_round_trips_within_a_step() {
        for v in [[0.0; 3], [0.5, -0.25, 0.125], [3.7, 0.0, -9.2], [-120.0, 64.5, 1.0]] {
            let mut buf = Vec::new();
            write_lp_vec3(v, &mut buf);
            let back = read_lp_vec3(&mut &buf[..]).unwrap();
            let step = v.iter().fold(1.0_f64, |m, c| m.max(c.abs().ceil())) * 2.0 / 32766.0;
            assert!(v.iter().zip(back).all(|(a, b)| (a - b).abs() <= step), "{v:?} came back as {back:?}");
        }
    }
}
