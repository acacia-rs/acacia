//! `.mcstructure` files: little-endian NBT with a size, a palette and per-cell palette indices (x outermost, z
//! innermost; -1 is structure void). Only the first (block) layer is read; the waterlog layer is ignored. Of the
//! block entities only sign text is kept (`block_position_data`, keyed by the cell's index).

use acacia_client::proto::nbt::{self, LittleEndian, List, Nbt, Value};

/// A palette entry, with properties in the block registry's form (`key=value`, sorted, comma separated).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Block {
    pub name: String,
    pub props: String,
}

impl Block {
    pub fn new(name: &str, props: &str) -> Self {
        let name = if name.contains(':') { name.to_string() } else { format!("minecraft:{name}") };
        let mut kv: Vec<&str> = props.split(',').filter(|s| !s.is_empty()).collect();
        kv.sort_unstable();
        Self { name, props: kv.join(",") }
    }

    pub fn prop(&self, key: &str) -> Option<&str> {
        self.props.split(',').find_map(|kv| kv.split_once('=').filter(|(k, _)| *k == key).map(|(_, v)| v))
    }

    pub fn is_air(&self) -> bool {
        self.name == "minecraft:air"
    }
}

/// The text on a sign's two sides (lines joined by `\n`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SignText {
    pub front: String,
    pub back: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Structure {
    pub size: [i32; 3],
    pub palette: Vec<Block>,
    /// Palette index per cell, -1 for structure void.
    cells: Vec<i32>,
    /// Sign text by cell index.
    signs: std::collections::BTreeMap<usize, SignText>,
}

#[derive(Debug, thiserror::Error)]
pub enum StructureError {
    #[error("not NBT: {0}")]
    Nbt(String),
    #[error("missing or malformed {0}")]
    Field(&'static str),
}

/// A bed's colour (its block entity's `color`, the item's aux) carried as a pseudo state of the bed's cells:
/// `rules::twocell` compares it from the world's block entities, the item key carries it.
pub const BED_COLOR: &str = "bed_color";

/// Palette `version` written with fixtures; BDS upgrades older states by it.
const BLOCK_VERSION: i32 = 18_168_865;

impl Structure {
    /// All structure void.
    pub fn new(size: [i32; 3]) -> Self {
        Self { size, palette: Vec::new(), cells: vec![-1; (size[0] * size[1] * size[2]).max(0) as usize], signs: Default::default() }
    }

    pub fn load(mut bytes: &[u8]) -> Result<Self, StructureError> {
        let root = nbt::read::<LittleEndian>(&mut bytes).map_err(|e| StructureError::Nbt(e.to_string()))?.value;
        let size: Vec<i32> = ints(root.get("size")).ok_or(StructureError::Field("size"))?;
        let size: [i32; 3] = size.try_into().map_err(|_| StructureError::Field("size"))?;
        let structure = root.get("structure").ok_or(StructureError::Field("structure"))?;
        let Some(Value::List(layers)) = structure.get("block_indices") else { return Err(StructureError::Field("block_indices")) };
        let cells = layers.items.first().and_then(|l| ints(Some(l))).ok_or(StructureError::Field("block_indices"))?;
        let palette = structure.get("palette").and_then(|p| p.get("default")).and_then(|d| d.get("block_palette"));
        let Some(Value::List(palette)) = palette else { return Err(StructureError::Field("block_palette")) };
        let palette = palette.items.iter().map(block).collect::<Option<Vec<_>>>().ok_or(StructureError::Field("block_palette"))?;
        if cells.len() != (size[0] * size[1] * size[2]).max(0) as usize || cells.iter().any(|&i| i >= palette.len() as i32 || i < -1) {
            return Err(StructureError::Field("block_indices"));
        }
        let data = structure.get("palette").and_then(|p| p.get("default")).and_then(|d| d.get("block_position_data"));
        let signs = match data {
            Some(Value::Compound(entries)) => entries.iter().filter_map(|(k, v)| Some((k.to_string().parse().ok()?, sign_text(v.get("block_entity_data")?)?))).collect(),
            _ => Default::default(),
        };
        let mut s = Self { size, palette, cells, signs };
        if let Some(Value::Compound(entries)) = data {
            for (k, v) in entries {
                let color = v.get("block_entity_data").and_then(|d| d.get("color"));
                let (Ok(i), Some(Value::Byte(c))) = (k.parse::<usize>(), color) else { continue };
                let pos = s.pos(i);
                if let Some(b) = s.get(pos).filter(|b| b.name == "minecraft:bed").cloned() {
                    s.set(pos, Some(Block::new(&b.name, &format!("{},{BED_COLOR}={}", b.props, *c as u8))));
                }
            }
        }
        Ok(s)
    }

    fn pos(&self, i: usize) -> [i32; 3] {
        let ([_, sy, sz], i) = (self.size, i as i32);
        [i / (sy * sz), (i / sz) % sy, i % sz]
    }

    /// `block_position_data`: a `Bed` block entity with its colour for every cell with [`BED_COLOR`], and each
    /// sign's text.
    fn position_data(&self) -> Value {
        let beds = self.cells.iter().enumerate().filter_map(|(i, &p)| {
            let c: u8 = self.palette.get(usize::try_from(p).ok()?)?.prop(BED_COLOR)?.parse().ok()?;
            let be = Value::Compound(vec![("id".into(), Value::String("Bed".into())), ("color".into(), Value::Byte(c as i8)), ("isMovable".into(), Value::Byte(1))]);
            Some((i.to_string().into(), Value::Compound(vec![("block_entity_data".into(), be)])))
        });
        let signs = self.signs.iter().map(|(i, t)| (i.to_string().into(), sign_nbt(t)));
        Value::Compound(beds.chain(signs).collect())
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let int_list = |v: &[i32]| Value::List(List { tag: 3, items: v.iter().map(|&i| Value::Int(i)).collect() });
        let palette = self.palette.iter().map(block_nbt).collect();
        let layer2 = vec![-1; self.cells.len()];
        let structure = Value::Compound(vec![
            ("block_indices".into(), Value::List(List { tag: 9, items: vec![int_list(&self.cells), int_list(&layer2)] })),
            ("entities".into(), Value::List(List { tag: 10, items: Vec::new() })),
            (
                "palette".into(),
                Value::Compound(vec![(
                    "default".into(),
                    Value::Compound(vec![
                        ("block_palette".into(), Value::List(List { tag: 10, items: palette })),
                        ("block_position_data".into(), self.position_data()),
                    ]),
                )]),
            ),
        ]);
        let root = Value::Compound(vec![
            ("format_version".into(), Value::Int(1)),
            ("size".into(), int_list(&self.size)),
            ("structure".into(), structure),
            ("structure_world_origin".into(), int_list(&[0, 0, 0])),
        ]);
        let mut out = bytes::BytesMut::new();
        nbt::write::<LittleEndian>(&mut out, &Nbt { name: String::new(), value: root });
        out.to_vec()
    }

    pub fn contains(&self, [x, y, z]: [i32; 3]) -> bool {
        (0..self.size[0]).contains(&x) && (0..self.size[1]).contains(&y) && (0..self.size[2]).contains(&z)
    }

    fn index(&self, [x, y, z]: [i32; 3]) -> Option<usize> {
        self.contains([x, y, z]).then(|| ((x * self.size[1] + y) * self.size[2] + z) as usize)
    }

    /// The block at a local position; `None` for structure void or outside.
    pub fn get(&self, pos: [i32; 3]) -> Option<&Block> {
        let i = *self.cells.get(self.index(pos)?)?;
        usize::try_from(i).ok().map(|i| &self.palette[i])
    }

    pub fn set(&mut self, pos: [i32; 3], block: Option<Block>) {
        let Some(at) = self.index(pos) else { return };
        self.cells[at] = match block {
            None => -1,
            Some(b) => match self.palette.iter().position(|p| *p == b) {
                Some(i) => i as i32,
                None => {
                    self.palette.push(b);
                    self.palette.len() as i32 - 1
                }
            },
        };
    }

    /// The sign text of a local cell, if the structure gives one.
    pub fn text(&self, pos: [i32; 3]) -> Option<&SignText> {
        self.signs.get(&self.index(pos)?)
    }

    pub fn set_text(&mut self, pos: [i32; 3], text: Option<SignText>) {
        let Some(i) = self.index(pos) else { return };
        match text {
            Some(t) => self.signs.insert(i, t),
            None => self.signs.remove(&i),
        };
    }

    /// Every non-void cell, local position first.
    pub fn cells(&self) -> impl Iterator<Item = ([i32; 3], &Block)> + '_ {
        let [_, sy, sz] = self.size;
        self.cells.iter().enumerate().filter_map(move |(i, &p)| {
            let i = i as i32;
            let pos = [i / (sy * sz), (i / sz) % sy, i % sz];
            usize::try_from(p).ok().map(|p| (pos, &self.palette[p]))
        })
    }
}

fn ints(v: Option<&Value>) -> Option<Vec<i32>> {
    let Some(Value::List(l)) = v else { return None };
    l.items.iter().map(|v| if let Value::Int(i) = v { Some(*i) } else { None }).collect()
}

fn block(v: &Value) -> Option<Block> {
    let Some(Value::String(name)) = v.get("name") else { return None };
    let mut props = Vec::new();
    if let Some(Value::Compound(states)) = v.get("states") {
        for (k, v) in states {
            let value = match v {
                Value::Byte(b) => b.to_string(),
                Value::Short(s) => s.to_string(),
                Value::Int(i) => i.to_string(),
                Value::String(s) => s.to_string(),
                _ => return None,
            };
            props.push(format!("{k}={value}"));
        }
    }
    Some(Block::new(name, &props.join(",")))
}

/// A sign's text from its block entity: `FrontText`/`BackText`, or the pre-1.20 single `Text`.
fn sign_text(v: &Value) -> Option<SignText> {
    let text = |v: Option<&Value>| match v {
        Some(Value::String(t)) => Some(t.to_string()),
        _ => None,
    };
    let (front, back, old) = (text(v.get("FrontText").and_then(|s| s.get("Text"))), text(v.get("BackText").and_then(|s| s.get("Text"))), text(v.get("Text")));
    (front.is_some() || back.is_some() || old.is_some()).then(|| SignText { front: front.or(old).unwrap_or_default(), back: back.unwrap_or_default() })
}

fn sign_nbt(t: &SignText) -> Value {
    let side = |text: &str| Value::Compound(vec![("Text".into(), Value::String(text.into()))]);
    let data = vec![("BackText".into(), side(&t.back)), ("FrontText".into(), side(&t.front)), ("id".into(), Value::String("Sign".into()))];
    Value::Compound(vec![("block_entity_data".into(), Value::Compound(data))])
}

/// Bits as bytes, numbers as ints, the rest as strings: how vanilla types block states.
fn block_nbt(b: &Block) -> Value {
    let states = b.props.split(',').filter_map(|kv| kv.split_once('=')).filter(|(k, _)| *k != BED_COLOR).map(|(k, v)| {
        let value = match v.parse::<i32>() {
            Ok(n) if k.ends_with("_bit") => Value::Byte(n as i8),
            Ok(n) => Value::Int(n),
            Err(_) => Value::String(v.into()),
        };
        (k.into(), value)
    });
    Value::Compound(vec![
        ("name".into(), Value::String(b.name.as_str().into())),
        ("states".into(), Value::Compound(states.collect())),
        ("version".into(), Value::Int(BLOCK_VERSION)),
    ])
}
