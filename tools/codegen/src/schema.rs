//! ProtoDef JSON → typed AST. No resolution happens here.

use serde_json::Value;

#[derive(Debug, Clone)]
pub enum Ty {
    Ref(String),
    Container(Vec<Field>),
    Array {
        count: Count,
        elem: Box<Ty>,
    },
    Option(Box<Ty>),
    Switch {
        compare_to: String,
        cases: Vec<(String, Ty)>,
        default: Box<Ty>,
    },
    Mapper {
        repr: Box<Ty>,
        mappings: Vec<(i64, String)>,
    },
    Bitflags {
        repr: Box<Ty>,
        flags: Vec<(String, u128)>,
    },
    Bitfield(Vec<BitfieldField>),
    Encapsulated {
        len: Box<Ty>,
        inner: Box<Ty>,
    },
    PString {
        count: Box<Ty>,
        latin1: bool,
    },
    Buffer {
        count: Box<Ty>,
    },
    MaybeIncompleteArray {
        count: Box<Ty>,
        elem: Box<Ty>,
    },
    OptionalOnRemaining(Box<Ty>),
}

#[derive(Debug, Clone)]
pub enum Count {
    Type(Box<Ty>),
    Fixed(usize),
    Field(String),
}

#[derive(Debug, Clone)]
pub struct Field {
    /// `None` for anonymous fields, whose contents ProtoDef merges into the parent.
    pub name: Option<String>,
    pub ty: Ty,
}

#[derive(Debug, Clone)]
pub struct BitfieldField {
    pub name: String,
    pub size: u32,
    pub signed: bool,
}

pub enum TypeDef {
    Native,
    Def(Ty),
}

pub struct Schema {
    pub types: Vec<(String, TypeDef)>,
}

impl Schema {
    pub fn parse(json: &Value) -> Schema {
        let types = json["types"]
            .as_object()
            .expect("protocol.json has no `types`");
        let types = types
            .iter()
            .map(|(name, v)| {
                let def = if v.as_str() == Some("native") {
                    TypeDef::Native
                } else {
                    TypeDef::Def(parse_ty(v))
                };
                (name.clone(), def)
            })
            .collect();
        Schema { types }
    }

    pub fn get(&self, name: &str) -> Option<&TypeDef> {
        self.types.iter().find(|(n, _)| n == name).map(|(_, d)| d)
    }
}

fn opt_ty(o: &Value, key: &str) -> Box<Ty> {
    Box::new(parse_ty(
        o.get(key)
            .unwrap_or_else(|| panic!("missing `{key}` in {o}")),
    ))
}

pub fn parse_ty(v: &Value) -> Ty {
    if let Some(s) = v.as_str() {
        return Ty::Ref(s.to_owned());
    }
    let arr = v.as_array().unwrap_or_else(|| panic!("bad type {v}"));
    let kind = arr[0].as_str().expect("type kind");
    let o = &arr[1];
    match kind {
        "container" => Ty::Container(
            o.as_array()
                .expect("container fields")
                .iter()
                .map(|f| Field {
                    name: f.get("name").and_then(Value::as_str).map(str::to_owned),
                    ty: parse_ty(&f["type"]),
                })
                .collect(),
        ),
        "array" => {
            let count = if let Some(ct) = o.get("countType") {
                Count::Type(Box::new(parse_ty(ct)))
            } else if let Some(n) = o["count"].as_u64() {
                Count::Fixed(n as usize)
            } else {
                Count::Field(o["count"].as_str().expect("array count").to_owned())
            };
            Ty::Array {
                count,
                elem: opt_ty(o, "type"),
            }
        }
        "option" => Ty::Option(Box::new(parse_ty(o))),
        "switch" => Ty::Switch {
            compare_to: o["compareTo"].as_str().expect("compareTo").to_owned(),
            cases: o["fields"]
                .as_object()
                .expect("switch fields")
                .iter()
                .map(|(k, v)| (k.clone(), parse_ty(v)))
                .collect(),
            default: Box::new(
                o.get("default")
                    .map(parse_ty)
                    .unwrap_or_else(|| Ty::Ref("void".into())),
            ),
        },
        "mapper" => Ty::Mapper {
            repr: opt_ty(o, "type"),
            mappings: o["mappings"]
                .as_object()
                .expect("mappings")
                .iter()
                .map(|(k, v)| (parse_int(k), v.as_str().expect("mapping name").to_owned()))
                .collect(),
        },
        "bitflags" => Ty::Bitflags {
            repr: opt_ty(o, "type"),
            flags: parse_flags(o),
        },
        "bitfield" => Ty::Bitfield(
            o.as_array()
                .expect("bitfield")
                .iter()
                .map(|f| BitfieldField {
                    name: f["name"].as_str().expect("bitfield name").to_owned(),
                    size: f["size"].as_u64().expect("bitfield size") as u32,
                    signed: f["signed"].as_bool().unwrap_or(false),
                })
                .collect(),
        ),
        "encapsulated" => Ty::Encapsulated {
            len: opt_ty(o, "lengthType"),
            inner: opt_ty(o, "type"),
        },
        "pstring" => Ty::PString {
            count: opt_ty(o, "countType"),
            latin1: o.get("encoding").and_then(Value::as_str) == Some("latin1"),
        },
        "buffer" => Ty::Buffer {
            count: opt_ty(o, "countType"),
        },
        "maybeIncompleteArray" => Ty::MaybeIncompleteArray {
            count: opt_ty(o, "countType"),
            elem: opt_ty(o, "type"),
        },
        "optionalOnRemaining" => Ty::OptionalOnRemaining(opt_ty(o, "type")),
        other => panic!("unsupported ProtoDef type kind `{other}`"),
    }
}

fn parse_int(k: &str) -> i64 {
    match k.strip_prefix("0x") {
        Some(hex) => i64::from_str_radix(hex, 16),
        None => k.parse(),
    }
    .unwrap_or_else(|_| panic!("mapper key `{k}` is not an integer"))
}

fn parse_flags(o: &Value) -> Vec<(String, u128)> {
    let shift = o.get("shift").and_then(Value::as_bool).unwrap_or(false);
    match &o["flags"] {
        Value::Array(names) => names
            .iter()
            .enumerate()
            .map(|(i, n)| (n.as_str().expect("flag").to_owned(), 1u128 << i))
            .collect(),
        Value::Object(map) => map
            .iter()
            .map(|(k, v)| {
                let n = v.as_u64().expect("flag value") as u128;
                (k.clone(), if shift { 1u128 << n } else { n })
            })
            .collect(),
        other => panic!("bad bitflags {other}"),
    }
}
