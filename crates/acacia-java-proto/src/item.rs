//! Item stacks (`ItemStack.OPTIONAL_STREAM_CODEC`) and their `DataComponentPatch`.

mod codecs;
pub mod components;
mod composites;
#[cfg(test)]
mod tests;

use crate::wire::read_len;
use crate::{Error, VarInt, Wire, nbt, read_varint, write_varint};

/// Patches nest through containers, bundles and predicates; the cap keeps a hostile packet off the stack's end.
const MAX_DEPTH: u32 = 256;

pub const MAX_STACK_SIZE: i32 = 1;
pub const MAX_DAMAGE: i32 = 2;
pub const DAMAGE: i32 = 3;
pub const UNBREAKABLE: i32 = 4;
pub const CUSTOM_NAME: i32 = 6;
pub const ENCHANTMENTS: i32 = 13;

/// A non-empty stack. On the wire an empty one is a count of 0, so slots read as `Option<ItemStack>`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ItemStack {
    /// `item` registry id.
    pub item: i32,
    pub count: i32,
    pub components: Components,
}

impl ItemStack {
    pub fn new(item: i32, count: i32) -> Self {
        Self { item, count, components: Components::default() }
    }
}

/// The parts of a stack's component patch the bot reads; the rest are skipped. Values the patch leaves out come from
/// the item's defaults, which the server never sends.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Components {
    pub max_stack_size: Option<i32>,
    pub max_damage: Option<i32>,
    pub damage: Option<i32>,
    pub unbreakable: bool,
    /// The text component as its network NBT; [`Components::custom_name_text`] reads a plain one.
    pub custom_name: Option<Vec<u8>>,
    /// `(enchantment registry id, level)`, in wire order.
    pub enchantments: Vec<(i32, i32)>,
    /// Component type ids the patch removes from the item's defaults.
    pub removed: Vec<i32>,
}

impl Components {
    pub fn custom_name_text(&self) -> Option<String> {
        nbt::read_root_string(self.custom_name.as_deref()?)
    }

    fn read_nested(input: &mut &[u8], depth: u32) -> Result<Self, Error> {
        if depth >= MAX_DEPTH {
            return Err(Error::TooDeep(MAX_DEPTH));
        }
        let (added, removed) = (read_len(input)?, read_len(input)?);
        let mut components = Self::default();
        for _ in 0..added {
            let id = read_varint(input)?;
            read_component(id, input, depth, &mut components)?;
        }
        for _ in 0..removed {
            let id = read_varint(input)?;
            if !(0..components::COUNT).contains(&id) {
                return Err(Error::Invalid("data component type"));
            }
            components.removed.push(id);
        }
        Ok(components)
    }

    /// A `DataComponentExactPredicate`: a list of typed components with no removals.
    pub(crate) fn read_exact(input: &mut &[u8]) -> Result<Self, Error> {
        let mut components = Self::default();
        for _ in 0..read_len(input)? {
            let id = read_varint(input)?;
            read_component(id, input, 0, &mut components)?;
        }
        Ok(components)
    }

    pub(crate) fn write_exact(&self, out: &mut Vec<u8>) {
        let added = self.added();
        write_varint(added.len() as i32, out);
        for (id, value) in added {
            write_varint(id, out);
            out.extend(value);
        }
    }

    fn added(&self) -> Vec<(i32, Vec<u8>)> {
        let mut added = Vec::new();
        let mut put = |id, value: &dyn Fn(&mut Vec<u8>)| {
            let mut out = Vec::new();
            value(&mut out);
            added.push((id, out));
        };
        if let Some(n) = self.max_stack_size {
            put(MAX_STACK_SIZE, &|out| write_varint(n, out));
        }
        if let Some(n) = self.max_damage {
            put(MAX_DAMAGE, &|out| write_varint(n, out));
        }
        if let Some(n) = self.damage {
            put(DAMAGE, &|out| write_varint(n, out));
        }
        if self.unbreakable {
            put(UNBREAKABLE, &|_| {});
        }
        if let Some(name) = &self.custom_name {
            put(CUSTOM_NAME, &|out| out.extend_from_slice(name));
        }
        if !self.enchantments.is_empty() {
            put(ENCHANTMENTS, &|out| {
                write_varint(self.enchantments.len() as i32, out);
                self.enchantments.iter().for_each(|&(id, level)| [id, level].iter().for_each(|&v| write_varint(v, out)));
            });
        }
        added
    }
}

impl Wire for Components {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        Self::read_nested(input, 0)
    }

    fn write(&self, out: &mut Vec<u8>) {
        let added = self.added();
        write_varint(added.len() as i32, out);
        write_varint(self.removed.len() as i32, out);
        for (id, value) in added {
            write_varint(id, out);
            out.extend(value);
        }
        self.removed.iter().for_each(|&id| write_varint(id, out));
    }
}

/// Reads component `id`'s value into `components` when the bot keeps it, else skips it.
fn read_component(id: i32, input: &mut &[u8], depth: u32, components: &mut Components) -> Result<(), Error> {
    match id {
        MAX_STACK_SIZE => components.max_stack_size = Some(read_varint(input)?),
        MAX_DAMAGE => components.max_damage = Some(read_varint(input)?),
        DAMAGE => components.damage = Some(read_varint(input)?),
        UNBREAKABLE => components.unbreakable = true,
        CUSTOM_NAME => {
            let start = *input;
            nbt::skip_tag(input)?;
            components.custom_name = Some(start[..start.len() - input.len()].to_vec());
        }
        ENCHANTMENTS => {
            components.enchantments.clear();
            for _ in 0..read_len(input)? {
                components.enchantments.push((read_varint(input)?, read_varint(input)?));
            }
        }
        _ => components::skip(id, input, depth)?,
    }
    Ok(())
}

impl Wire for Option<ItemStack> {
    fn read(input: &mut &[u8]) -> Result<Self, Error> {
        let count = read_varint(input)?;
        if count <= 0 {
            return Ok(None);
        }
        let item = read_varint(input)?;
        Ok(Some(ItemStack { item, count, components: Components::read(input)? }))
    }

    fn write(&self, out: &mut Vec<u8>) {
        match self {
            Some(stack) if stack.count > 0 => {
                write_varint(stack.count, out);
                VarInt(stack.item).write(out);
                stack.components.write(out);
            }
            _ => write_varint(0, out),
        }
    }
}
