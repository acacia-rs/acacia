//! Container clicks and what they carry: the client's predicted slots as `HashedStack`s, hashed with
//! `HashOps.CRC32C_INSTANCE`. A wrong hash only makes the server resend that slot.

use crc::{CRC_32_ISCSI, Crc};

use crate::item::{CUSTOM_NAME, DAMAGE, ENCHANTMENTS, ItemStack, MAP_ID, MAX_DAMAGE, MAX_STACK_SIZE, UNBREAKABLE};
use crate::nbt;
use crate::wire::read_len;
use crate::{Error, VarInt, Wire, write_varint};

const CRC32C: Crc<u32> = Crc::<u32>::new(&CRC_32_ISCSI);
/// `HashOps` tags.
const TAG_MAP_START: u8 = 2;
const TAG_MAP_END: u8 = 3;
const TAG_INT: u8 = 8;
const TAG_STRING: u8 = 12;

/// `ContainerInput`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerInput {
    Pickup,
    QuickMove,
    Swap,
    Clone,
    Throw,
    QuickCraft,
    PickupAll,
}

/// `HashOps.createInt`: Guava hashes the tag then the int little-endian.
fn hash_int(v: i32) -> i32 {
    let mut digest = CRC32C.digest();
    digest.update(&[TAG_INT]);
    digest.update(&v.to_le_bytes());
    digest.finalize() as i32
}

/// A `Unit` encodes as an empty map.
fn hash_unit() -> i32 {
    hash_map(Vec::new())
}

/// `HashOps.createString`: the tag, the length in UTF-16 units, then the units (`putUnencodedChars`), little-endian.
fn hash_string(s: &str) -> i32 {
    let units: Vec<u16> = s.encode_utf16().collect();
    let mut digest = CRC32C.digest();
    digest.update(&[TAG_STRING]);
    digest.update(&(units.len() as i32).to_le_bytes());
    units.iter().for_each(|u| digest.update(&u.to_le_bytes()));
    digest.finalize() as i32
}

/// `HashOps.createMap`: entries' (key, value) hashes sorted as unsigned ints (`padToLong`), key first.
fn hash_map(mut entries: Vec<(i32, i32)>) -> i32 {
    entries.sort_by_key(|&(k, v)| (k as u32, v as u32));
    let mut digest = CRC32C.digest();
    digest.update(&[TAG_MAP_START]);
    for (k, v) in entries {
        digest.update(&k.to_le_bytes());
        digest.update(&v.to_le_bytes());
    }
    digest.update(&[TAG_MAP_END]);
    digest.finalize() as i32
}

/// `HashedStack.ActualItem`; `None` on the wire is the empty stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashedStack {
    pub item: i32,
    pub count: i32,
    /// (component type id, hash), for the components set by the stack's patch.
    pub added: Vec<(i32, i32)>,
    pub removed: Vec<i32>,
}

impl HashedStack {
    /// The stack as the client would hash it, with `enchantments` the enchantment registry's names by id.
    /// Components the client does not decode (or a styled name) are left out, so such a stack's hash misses and the
    /// server resends it.
    pub fn of(stack: &ItemStack, enchantments: &[String]) -> HashedStack {
        let c = &stack.components;
        let ints = [(MAX_STACK_SIZE, c.max_stack_size), (MAX_DAMAGE, c.max_damage), (DAMAGE, c.damage), (MAP_ID, c.map_id)];
        let mut added: Vec<_> = ints.iter().filter_map(|&(id, v)| Some((id, hash_int(v?)))).collect();
        if c.unbreakable {
            added.push((UNBREAKABLE, hash_unit()));
        }
        // A plain name's codec collapses it to a string; enchantments are a map of registry key to level.
        if let Some(name) = c.custom_name_text() {
            added.push((CUSTOM_NAME, hash_string(&name)));
        }
        let named = |&(id, level): &(i32, i32)| Some((hash_string(enchantments.get(usize::try_from(id).ok()?)?), hash_int(level)));
        if let Some(entries) = c.enchantments.iter().map(named).collect::<Option<Vec<_>>>().filter(|e| !e.is_empty()) {
            added.push((ENCHANTMENTS, hash_map(entries)));
        }
        HashedStack { item: stack.item, count: stack.count, added, removed: c.removed.clone() }
    }
}

impl Wire for Option<HashedStack> {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        if !bool::read(input)? {
            return Ok(None);
        }
        let (item, count) = (VarInt::read(input)?.0, VarInt::read(input)?.0);
        let mut added = Vec::new();
        for _ in 0..read_len(input)? {
            added.push((VarInt::read(input)?.0, i32::read(input)?));
        }
        let mut removed = Vec::new();
        for _ in 0..read_len(input)? {
            removed.push(VarInt::read(input)?.0);
        }
        Ok(Some(HashedStack { item, count, added, removed }))
    }

    fn write(&self, out: &mut Vec<u8>) {
        let Some(s) = self else { return false.write(out) };
        true.write(out);
        write_varint(s.item, out);
        write_varint(s.count, out);
        write_varint(s.added.len() as i32, out);
        for &(id, hash) in &s.added {
            write_varint(id, out);
            hash.write(out);
        }
        write_varint(s.removed.len() as i32, out);
        s.removed.iter().for_each(|&id| write_varint(id, out));
    }
}

/// `ServerboundContainerClickPacket`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerClick {
    pub container: i32,
    pub state_id: i32,
    pub slot: i16,
    pub button: i8,
    pub input: ContainerInput,
    /// The slots the client predicts changed, with their new contents.
    pub changed: Vec<(i16, Option<HashedStack>)>,
    pub carried: Option<HashedStack>,
}

impl Wire for ContainerClick {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let (container, state_id) = (VarInt::read(input)?.0, VarInt::read(input)?.0);
        let (slot, button) = (i16::read(input)?, i8::read(input)?);
        let inputs = [ContainerInput::Pickup, ContainerInput::QuickMove, ContainerInput::Swap, ContainerInput::Clone, ContainerInput::Throw, ContainerInput::QuickCraft, ContainerInput::PickupAll];
        let kind = usize::try_from(VarInt::read(input)?.0).ok().and_then(|i| inputs.get(i).copied()).ok_or(Error::Invalid("container input"))?;
        let mut changed = Vec::new();
        for _ in 0..read_len(input)? {
            changed.push((i16::read(input)?, Option::<HashedStack>::read(input)?));
        }
        Ok(Self { container, state_id, slot, button, input: kind, changed, carried: Option::<HashedStack>::read(input)? })
    }

    fn write(&self, out: &mut Vec<u8>) {
        write_varint(self.container, out);
        write_varint(self.state_id, out);
        self.slot.write(out);
        self.button.write(out);
        write_varint(self.input as i32, out);
        write_varint(self.changed.len() as i32, out);
        for (slot, stack) in &self.changed {
            slot.write(out);
            stack.write(out);
        }
        self.carried.write(out);
    }
}

crate::packets::packet!(ContainerClose { container: VarInt });
// A window property: a furnace's fuel and cooking timers, a brewing stand's brew time, ...
crate::packets::packet!(ContainerSetData { container: VarInt, id: i16, value: i16 });

/// `ClientboundOpenScreenPacket`: `menu` is a `minecraft:menu` registry id, `title` a text component's network NBT.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenScreen {
    pub container: i32,
    pub menu: i32,
    pub title: Vec<u8>,
}

impl Wire for OpenScreen {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let (container, menu) = (VarInt::read(input)?.0, VarInt::read(input)?.0);
        let start = *input;
        nbt::skip_tag(input)?;
        Ok(Self { container, menu, title: start[..start.len() - input.len()].to_vec() })
    }

    fn write(&self, out: &mut Vec<u8>) {
        write_varint(self.container, out);
        write_varint(self.menu, out);
        out.extend_from_slice(&self.title);
    }
}

impl OpenScreen {
    /// The title if it is plain text (renamed blocks); default titles are translations and give `None`.
    pub fn title_text(&self) -> Option<String> {
        nbt::read_root_string(&self.title)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_match_guava_crc32c() {
        // Guava: Hashing.crc32c().newHasher().putByte(8).putInt(5).hash().asInt()
        assert_eq!(hash_int(5), CRC32C.checksum(&[8, 5, 0, 0, 0]) as i32);
        assert_eq!(CRC32C.checksum(b"123456789"), 0xE306_9283);
        assert_eq!(hash_string("ab"), CRC32C.checksum(&[12, 2, 0, 0, 0, b'a', 0, b'b', 0]) as i32);
    }

    #[test]
    fn enchantments_hash_as_a_map_sorted_by_unsigned_key_hash() {
        let names = ["minecraft:efficiency".to_owned(), "minecraft:unbreaking".to_owned()];
        let mut pickaxe = ItemStack::new(1052, 1);
        pickaxe.components.enchantments = vec![(1, 3), (0, 5)];
        let entries = [(hash_string(&names[0]), hash_int(5)), (hash_string(&names[1]), hash_int(3))];
        let mut sorted = entries.to_vec();
        sorted.sort_by_key(|&(k, _)| k as u32);
        let mut bytes = vec![TAG_MAP_START];
        sorted.iter().for_each(|(k, v)| bytes.extend(k.to_le_bytes().into_iter().chain(v.to_le_bytes())));
        bytes.push(TAG_MAP_END);
        assert_eq!(HashedStack::of(&pickaxe, &names).added, vec![(ENCHANTMENTS, CRC32C.checksum(&bytes) as i32)]);
        // An id the registry does not name leaves the component out: the server resends the slot.
        assert!(HashedStack::of(&pickaxe, &names[..1]).added.is_empty());
    }

    #[test]
    fn open_screen_keeps_its_title_nbt() {
        let title = vec![nbt::TAG_STRING, 0, 5, b'C', b'h', b'e', b's', b't'];
        let screen = OpenScreen { container: 3, menu: 2, title };
        let mut buf = Vec::new();
        screen.write(&mut buf);
        buf.push(0xff);
        let mut input = &buf[..];
        assert_eq!(OpenScreen::read(&mut input), Ok(screen.clone()));
        assert_eq!((input, screen.title_text().as_deref()), (&[0xff][..], Some("Chest")));
    }

    #[test]
    fn clicks_round_trip() {
        let mut pickaxe = ItemStack::new(1052, 1);
        pickaxe.components.damage = Some(100);
        let click = ContainerClick {
            container: 0,
            state_id: 7,
            slot: 36,
            button: 0,
            input: ContainerInput::Pickup,
            changed: vec![(36, None)],
            carried: Some(HashedStack::of(&pickaxe, &[])),
        };
        let mut buf = Vec::new();
        click.write(&mut buf);
        assert_eq!(ContainerClick::read(&mut &buf[..]), Ok(click));
    }
}
