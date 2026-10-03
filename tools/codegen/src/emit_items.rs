//! Rust item definitions (structs, enums, flags) with their `read`/`write` impls.

use crate::emit_read::Ctx;
use crate::ir::*;

impl Ctx<'_> {
    pub fn item(&self, id: ItemId) -> String {
        let item = &self.items[id];
        let name = &item.name;
        match &item.kind {
            ItemKind::Struct { fields } => self.struct_item(item, fields),
            ItemKind::Mapper { repr, variants } => mapper(name, *repr, variants),
            ItemKind::Bitflags { repr, flags } => bitflags(name, *repr, flags),
            ItemKind::Bitfield { fields } => bitfield(name, fields),
            ItemKind::Switch { variants } => {
                let vs: String = variants
                    .iter()
                    .map(|v| match &v.ty {
                        Some(t) => format!("{}({}),\n", v.name, self.rust_ty(t)),
                        None => format!("{},\n", v.name),
                    })
                    .collect();
                format!("#[derive(Debug, Clone, PartialEq)]\npub enum {name} {{\n{vs}}}\n")
            }
            ItemKind::Alias { target } => format!("pub type {name} = {};\n", self.rust_ty(target)),
            ItemKind::Pending => panic!("item {name} was never filled in"),
        }
    }

    fn struct_item(&self, item: &Item, fields: &[IrField]) -> String {
        let name = &item.name;
        let defs: String = fields
            .iter()
            .map(|f| format!("pub {}: {},\n", f.name, self.rust_ty(&f.ty)))
            .collect();
        let mut s = format!("#[derive(Debug, Clone, PartialEq)]\npub struct {name} {{\n{defs}}}\n");
        if !item.standalone {
            return s;
        }
        let reads: String = fields
            .iter()
            .map(|f| {
                let at = format!("{name}.{}", f.name.trim_start_matches("r#"));
                format!(
                    "let {} = (|| -> Result<_> {{ Ok({}) }})().map_err(|e| e.at({at:?}))?;\n",
                    f.var,
                    self.read(&f.ty)
                )
            })
            .collect();
        let inits: Vec<String> = fields
            .iter()
            .map(|f| format!("{}: {}", f.name, f.var))
            .collect();
        let writes: String = fields
            .iter()
            .map(|f| self.write(&f.ty, &format!("&self.{}", f.name), 0) + "\n")
            .collect();
        s += &format!(
            "impl {name} {{\npub fn read(r: &mut &[u8]) -> Result<Self> {{\n{reads}Ok(Self {{ {} }})\n}}\n\
             pub fn write(&self, w: &mut BytesMut) {{\n{writes}}}\n}}\n",
            inits.join(", ")
        );
        if let Some((pid, pname)) = &item.packet {
            s += &format!(
                "impl crate::Packet for {name} {{\nconst ID: u32 = {pid};\nconst NAME: &'static str = {pname:?};\n\
                 fn encode(&self, w: &mut BytesMut) {{ self.write(w) }}\n\
                 fn decode(r: &mut &[u8]) -> Result<Self> {{ Self::read(r) }}\n}}\n"
            );
        }
        s
    }
}

fn mapper(name: &str, repr: Prim, variants: &[(i64, String, String)]) -> String {
    let t = repr.rust();
    let c = repr.codec();
    let defs: String = variants.iter().map(|(_, _, v)| format!("{v},\n")).collect();
    let from: String = variants
        .iter()
        .map(|(k, _, v)| format!("{k} => Self::{v},\n"))
        .collect();
    let to: String = variants
        .iter()
        .map(|(k, _, v)| format!("Self::{v} => {k},\n"))
        .collect();
    format!(
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\npub enum {name} {{\n{defs}Unknown(i64),\n}}\n\
         impl {name} {{\n\
         pub fn from_raw(v: i64) -> Self {{ match v {{\n{from}v => Self::Unknown(v),\n}} }}\n\
         pub fn to_raw(self) -> i64 {{ match self {{\n{to}Self::Unknown(v) => v,\n}} }}\n\
         pub fn read(r: &mut &[u8]) -> Result<Self> {{ Ok(Self::from_raw(read_{c}(r)? as i64)) }}\n\
         pub fn write(&self, w: &mut BytesMut) {{ write_{c}(w, self.to_raw() as {t}) }}\n}}\n"
    )
}

fn bitflags(name: &str, repr: Prim, flags: &[(String, u128)]) -> String {
    let t = repr.rust();
    let c = repr.codec();
    let consts: String = flags
        .iter()
        .map(|(n, v)| format!("pub const {n}: Self = Self({v}u128 as {t});\n"))
        .collect();
    format!(
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]\npub struct {name}(pub {t});\n\
         impl {name} {{\n{consts}\
         pub fn contains(self, other: Self) -> bool {{ self.0 & other.0 == other.0 }}\n\
         pub fn insert(&mut self, other: Self) {{ self.0 |= other.0 }}\n\
         pub fn remove(&mut self, other: Self) {{ self.0 &= !other.0 }}\n\
         pub fn read(r: &mut &[u8]) -> Result<Self> {{ Ok(Self(read_{c}(r)?)) }}\n\
         pub fn write(&self, w: &mut BytesMut) {{ write_{c}(w, self.0) }}\n}}\n"
    )
}

/// ProtoDef bitfields pack fields MSB-first into a big-endian byte run.
fn bitfield(name: &str, fields: &[(String, u32, bool)]) -> String {
    let total: u32 = fields.iter().map(|f| f.1).sum();
    let bytes = total.div_ceil(8);
    let ty = |size: u32, signed: bool| {
        let bits = [8, 16, 32, 64]
            .into_iter()
            .find(|b| *b >= size)
            .expect("bitfield field too wide");
        format!("{}{bits}", if signed { 'i' } else { 'u' })
    };
    let defs: String = fields
        .iter()
        .map(|(n, s, sg)| format!("pub {n}: {},\n", ty(*s, *sg)))
        .collect();
    let mut shift = bytes * 8;
    let (mut reads, mut writes) = (String::new(), String::new());
    for (n, size, signed) in fields {
        shift -= size;
        let mask = (1u64 << size) - 1;
        let t = ty(*size, *signed);
        let ext = if *signed {
            format!(
                "let v = ((v << {sh}) as i64 >> {sh}) as u64; ",
                sh = 64 - size
            )
        } else {
            String::new()
        };
        reads += &format!("{n}: {{ let v = (bits >> {shift}) & {mask}; {ext}v as {t} }},\n");
        writes += &format!("bits |= ((self.{n} as u64) & {mask}) << {shift};\n");
    }
    format!(
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]\npub struct {name} {{\n{defs}}}\n\
         impl {name} {{\n\
         pub fn read(r: &mut &[u8]) -> Result<Self> {{\n\
         let bits = take(r, {bytes})?.iter().fold(0u64, |acc, b| (acc << 8) | *b as u64);\n\
         Ok(Self {{\n{reads}}})\n}}\n\
         pub fn write(&self, w: &mut BytesMut) {{\nlet mut bits = 0u64;\n{writes}\
         write_slice(w, &bits.to_be_bytes()[{skip}..]);\n}}\n}}\n",
        skip = 8 - bytes
    )
}
