//! Resolved intermediate representation that the emitters consume.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prim {
    VarInt,
    VarInt64,
    VarInt128,
    ZigZag32,
    ZigZag64,
    U8,
    I8,
    LU16,
    LI16,
    LU32,
    LI32,
    LU64,
    LI64,
    LF32,
    LF64,
    U16,
    I16,
    U32,
    I32,
    U64,
    I64,
    F32,
    F64,
    Bool,
    ByteRot,
}

impl Prim {
    pub fn from_name(n: &str) -> Option<Prim> {
        use Prim::*;
        Some(match n {
            "varint" => VarInt,
            "varint64" => VarInt64,
            "varint128" => VarInt128,
            "zigzag32" => ZigZag32,
            "zigzag64" => ZigZag64,
            "u8" => U8,
            "i8" => I8,
            "lu16" => LU16,
            "li16" => LI16,
            "lu32" => LU32,
            "li32" => LI32,
            "lu64" => LU64,
            "li64" => LI64,
            "lf32" => LF32,
            "lf64" => LF64,
            "u16" => U16,
            "i16" => I16,
            "u32" => U32,
            "i32" => I32,
            "u64" => U64,
            "i64" => I64,
            "f32" => F32,
            "f64" => F64,
            "bool" => Bool,
            "byterot" => ByteRot,
            _ => return None,
        })
    }

    /// Suffix of the `codec::read_*` / `codec::write_*` functions.
    pub fn codec(self) -> &'static str {
        use Prim::*;
        match self {
            VarInt => "varint",
            VarInt64 => "varint64",
            VarInt128 => "varint128",
            ZigZag32 => "zigzag32",
            ZigZag64 => "zigzag64",
            U8 => "u8",
            I8 => "i8",
            LU16 => "lu16",
            LI16 => "li16",
            LU32 => "lu32",
            LI32 => "li32",
            LU64 => "lu64",
            LI64 => "li64",
            LF32 => "lf32",
            LF64 => "lf64",
            U16 => "u16",
            I16 => "i16",
            U32 => "u32",
            I32 => "i32",
            U64 => "u64",
            I64 => "i64",
            F32 => "f32",
            F64 => "f64",
            Bool => "bool",
            ByteRot => "u8",
        }
    }

    pub fn rust(self) -> &'static str {
        use Prim::*;
        match self {
            VarInt | LU32 | U32 => "u32",
            VarInt64 | LU64 | U64 => "u64",
            VarInt128 => "u128",
            ZigZag32 | LI32 | I32 => "i32",
            ZigZag64 | LI64 | I64 => "i64",
            U8 => "u8",
            I8 => "i8",
            LU16 | U16 => "u16",
            LI16 | I16 => "i16",
            LF32 | F32 | ByteRot => "f32",
            LF64 | F64 => "f64",
            Bool => "bool",
        }
    }

    pub fn is_int(self) -> bool {
        !matches!(
            self,
            Prim::LF32 | Prim::LF64 | Prim::F32 | Prim::F64 | Prim::Bool | Prim::ByteRot
        )
    }
}

pub type ItemId = usize;

#[derive(Debug, Clone)]
pub enum IrTy {
    Prim(Prim),
    Void,
    Str {
        count: Prim,
        latin1: bool,
    },
    Buf {
        count: Prim,
    },
    Rest,
    Uuid,
    Nbt {
        le: bool,
    },
    NbtLoop,
    /// Self-contained item with its own `read`/`write` methods.
    Item(ItemId),
    /// Struct decoded in place, because its fields may reference the enclosing scope.
    Inline(ItemId),
    Alias {
        item: ItemId,
        target: Box<IrTy>,
    },
    Array {
        count: IrCount,
        elem: Box<IrTy>,
    },
    Fixed {
        n: usize,
        elem: Box<IrTy>,
    },
    Option(Box<IrTy>),
    Switch {
        disc: Disc,
        item: ItemId,
    },
    /// A switch with one non-void arm: `Some` when the discriminant matches `keys` (or doesn't, if `negate`).
    SwitchOpt {
        disc: Disc,
        keys: Vec<String>,
        negate: bool,
        inner: Box<IrTy>,
    },
    Encapsulated {
        len: Prim,
        inner: Box<IrTy>,
    },
    MaybeIncomplete {
        count: Prim,
        elem: Box<IrTy>,
    },
    OnRemaining(Box<IrTy>),
}

#[derive(Debug, Clone)]
pub enum IrCount {
    Prim(Prim),
    /// Count taken from an earlier field; nothing is written for it on encode.
    Var(String),
}

#[derive(Debug, Clone)]
pub struct Disc {
    pub var: String,
    pub kind: DiscKind,
    /// The discriminant field is an `option`; an absent value matches no key.
    pub optional: bool,
}

#[derive(Debug, Clone)]
pub enum DiscKind {
    Enum(ItemId),
    Bool,
    Int(Prim),
    Str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Module {
    Types,
    Packets,
}

#[derive(Debug, Clone)]
pub struct IrField {
    pub name: String,
    /// Local variable holding the decoded value, visible to later switches and counts.
    pub var: String,
    pub ty: IrTy,
}

#[derive(Debug, Clone)]
pub struct Variant {
    pub name: String,
    /// Discriminant key this variant is selected by; `None` for the default arm.
    pub key: Option<String>,
    pub ty: Option<IrTy>,
}

#[derive(Debug, Clone)]
pub enum ItemKind {
    Struct {
        fields: Vec<IrField>,
    },
    Mapper {
        repr: Prim,
        variants: Vec<(i64, String, String)>,
    },
    Bitflags {
        repr: Prim,
        flags: Vec<(String, u128)>,
    },
    Bitfield {
        fields: Vec<(String, u32, bool)>,
    },
    Switch {
        variants: Vec<Variant>,
    },
    Alias {
        target: IrTy,
    },
    Pending,
}

#[derive(Debug, Clone)]
pub struct Item {
    pub name: String,
    pub module: Module,
    pub kind: ItemKind,
    /// Index of the top-level type/packet this item was hoisted from; used to group output files.
    pub root: usize,
    pub packet: Option<(u32, String)>,
    /// Struct gets its own `read`/`write` (top-level types and packets); inline structs are decoded in place.
    pub standalone: bool,
}

impl Item {
    /// Variant name of a mapper enum for a ProtoDef value name.
    pub fn mapper_variant(&self, proto: &str) -> Option<&str> {
        match &self.kind {
            ItemKind::Mapper { variants, .. } => variants
                .iter()
                .find(|(_, p, _)| p == proto)
                .map(|(_, _, n)| n.as_str()),
            _ => None,
        }
    }
}
