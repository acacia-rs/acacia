pub(super) fn var_u32(out: &mut Vec<u8>, mut v: u32) {
    while v >= 0x80 {
        out.push(v as u8 | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

pub(super) fn var_i32(out: &mut Vec<u8>, v: i32) {
    var_u32(out, ((v << 1) ^ (v >> 31)) as u32);
}
