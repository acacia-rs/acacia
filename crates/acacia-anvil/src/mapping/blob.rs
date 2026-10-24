//! Parser for `data/mapping.bin` (layout in tools/gen-mapping.mjs).

use super::{JavaBlock, Tables};

struct Reader<'a> {
    b: &'a [u8],
    p: usize,
}

impl Reader<'_> {
    fn bytes(&mut self, n: usize) -> &[u8] {
        let s = &self.b[self.p..self.p + n];
        self.p += n;
        s
    }
    fn u8(&mut self) -> u8 {
        self.bytes(1)[0]
    }
    fn u16(&mut self) -> u16 {
        u16::from_le_bytes(self.bytes(2).try_into().unwrap())
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.bytes(4).try_into().unwrap())
    }
}

pub(super) fn parse(b: &'static [u8]) -> Tables {
    assert_eq!(&b[..4], b"AJM1", "mapping.bin magic");
    let mut r = Reader { b, p: 4 };
    let n = r.u32() as usize;
    let mut strings: Vec<&'static str> = Vec::with_capacity(n);
    for _ in 0..n {
        let len = r.u16() as usize;
        let start = r.p;
        r.p += len;
        strings.push(std::str::from_utf8(&b[start..start + len]).expect("utf8"));
    }
    let str_at = |i: u32| strings[i as usize];

    let n = r.u32() as usize;
    let mut blocks = Vec::with_capacity(n);
    let mut base = 0u32;
    for _ in 0..n {
        let name = str_at(r.u32());
        let default_index = r.u32();
        let mut props = Vec::new();
        for _ in 0..r.u8() {
            let key = str_at(r.u32());
            let values: Vec<&'static str> = (0..r.u8()).map(|_| str_at(r.u32())).collect();
            props.push((key, values.into_boxed_slice()));
        }
        let block = JavaBlock {
            name,
            base,
            default_index,
            props: props.into_boxed_slice(),
        };
        base += block.state_count();
        blocks.push(block);
    }

    let n = r.u32() as usize;
    let mut bedrock = Vec::with_capacity(n);
    for _ in 0..n {
        let (name, props, java) = (str_at(r.u32()), str_at(r.u32()), r.u32());
        bedrock.push((name, props, java));
    }
    let n = r.u32() as usize;
    assert_eq!(n as u32, base, "forward map length");
    let forward: Vec<u16> = (0..n).map(|_| r.u16()).collect();

    let n = r.u32() as usize;
    let mut biomes = Vec::with_capacity(n);
    for _ in 0..n {
        let (id, name) = (r.u32(), str_at(r.u32()));
        biomes.push((id, name));
    }
    assert_eq!(r.p, b.len(), "mapping.bin trailing bytes");
    Tables {
        blocks,
        state_count: base,
        bedrock,
        forward,
        biomes,
    }
}
