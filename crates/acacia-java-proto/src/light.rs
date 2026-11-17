//! `ClientboundLightUpdatePacketData` (in `LightUpdate` and after a chunk's block entities): the sky layer's
//! sections. Block light is read past.

use crate::nbt::skip_root;
use crate::{Error, Wire, read_varint};

/// What the packet sets a light section to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LightSection {
    /// `new DataLayer()`: no light anywhere.
    Empty,
    /// A `DataLayer`'s nibbles, two cells a byte (low nibble first) in `y z x` order.
    Data(Box<[u8]>),
}

/// `ByteBufCodecs.BIT_SET` is `BitSet.toByteArray`: little-endian bytes.
fn bit(mask: &[u8], i: usize) -> bool {
    mask.get(i / 8).is_some_and(|b| (b >> (i % 8)) & 1 == 1)
}

/// The sky sections the packet sets, by index from the light section below the lowest block section, in order.
pub fn read_sky_light(input: &mut &[u8]) -> Result<Vec<(usize, LightSection)>, Error> {
    let sky = <Vec<u8>>::read(input)?;
    <Vec<u8>>::read(input)?;
    let empty_sky = <Vec<u8>>::read(input)?;
    <Vec<u8>>::read(input)?;
    let mut layers = <Vec<Vec<u8>>>::read(input)?.into_iter();
    <Vec<Vec<u8>>>::read(input)?;
    let mut sections = Vec::new();
    for i in 0..sky.len().max(empty_sky.len()) * 8 {
        // `readSectionList`: data wins over the empty bit.
        if bit(&sky, i) {
            let data = layers.next().ok_or(Error::Invalid("sky light layer count"))?;
            sections.push((i, LightSection::Data(data.into())));
        } else if bit(&empty_sky, i) {
            sections.push((i, LightSection::Empty));
        }
    }
    Ok(sections)
}

/// `BlockEntityInfo`s after a chunk's sections: packed x/z, y, type and an optional compound.
pub fn skip_block_entities(input: &mut &[u8]) -> Result<(), Error> {
    for _ in 0..read_varint(input)? {
        u8::read(input)?;
        i16::read(input)?;
        read_varint(input)?;
        skip_root(input)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::write_varint;

    #[test]
    fn data_sections_take_layers_in_order_and_empty_ones_none() {
        let mut buf = Vec::new();
        let mask = |out: &mut Vec<u8>, bits: u8| {
            write_varint(1, out);
            out.push(bits);
        };
        mask(&mut buf, 0b1010);
        mask(&mut buf, 0);
        mask(&mut buf, 0b0101);
        mask(&mut buf, 0);
        write_varint(2, &mut buf);
        for fill in [0xff_u8, 0x0f] {
            write_varint(2048, &mut buf);
            buf.extend([fill; 2048]);
        }
        write_varint(0, &mut buf);
        let sections = read_sky_light(&mut &buf[..]).unwrap();
        let kinds: Vec<_> = sections.iter().map(|(i, s)| (*i, matches!(s, LightSection::Data(d) if d[0] == if *i == 1 { 0xff } else { 0x0f }))).collect();
        assert_eq!(kinds, [(0, false), (1, true), (2, false), (3, true)]);
    }
}
