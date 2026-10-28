//! Where font files are read from, as `VanillaPackResources` layers them: a namespace directory
//! (the jar's `assets/minecraft`, or a Java look's files in the same layout) and Mojang's asset
//! index (`indexes/<id>.json` naming `objects/<hh>/<hash>`), the first layer holding a path winning.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub enum Layer {
    /// `dir/<path>` is `<namespace>:<path>`.
    Namespace { namespace: String, dir: PathBuf },
    /// `<namespace>/<path>` keys of an asset index, each an object file.
    Index(BTreeMap<String, PathBuf>),
}

#[derive(Default)]
pub struct Resources {
    pub layers: Vec<Layer>,
}

/// `minecraft:font/x.json` as `("minecraft", "font/x.json")`.
pub fn split_id(id: &str) -> (&str, &str) {
    id.split_once(':').unwrap_or(("minecraft", id))
}

impl Resources {
    /// A namespace directory alone.
    pub fn dir(namespace: &str, dir: impl Into<PathBuf>) -> Resources {
        Resources { layers: vec![Layer::Namespace { namespace: namespace.to_owned(), dir: dir.into() }] }
    }

    /// The game's resources: `index` (an asset index root holding `indexes/` and `objects/`, when it
    /// has `indexes/<id>.json`) over the jar's `assets/minecraft`.
    pub fn java(assets: &Path, index: Option<(&Path, &str)>) -> Resources {
        let mut layers = Vec::new();
        if let Some(layer) = index.and_then(|(root, id)| Layer::index(root, id)) {
            layers.push(layer);
        }
        layers.push(Layer::Namespace { namespace: "minecraft".into(), dir: assets.to_owned() });
        Resources { layers }
    }

    pub fn read(&self, id: &str) -> Option<Vec<u8>> {
        let (ns, path) = split_id(id);
        self.layers.iter().find_map(|layer| std::fs::read(layer.path(ns, path)?).ok())
    }

    /// Every `<ns>:<prefix>/...<suffix>` id, sorted.
    pub fn list(&self, prefix: &str, suffix: &str) -> Vec<String> {
        let mut out = std::collections::BTreeSet::new();
        for layer in &self.layers {
            match layer {
                Layer::Namespace { namespace, dir } => walk(&dir.join(prefix), &format!("{namespace}:{prefix}/"), suffix, &mut out),
                Layer::Index(objects) => {
                    for key in objects.keys() {
                        let Some((ns, path)) = key.split_once('/') else { continue };
                        if path.starts_with(prefix) && path[prefix.len()..].starts_with('/') && path.ends_with(suffix) {
                            out.insert(format!("{ns}:{path}"));
                        }
                    }
                }
            }
        }
        out.into_iter().collect()
    }
}

impl Layer {
    /// An asset index's objects, `None` without its index file.
    pub fn index(root: &Path, id: &str) -> Option<Layer> {
        let index: serde_json::Value = serde_json::from_slice(&std::fs::read(root.join(format!("indexes/{id}.json"))).ok()?).ok()?;
        let objects = index["objects"].as_object()?.iter().filter_map(|(key, o)| {
            let hash = o["hash"].as_str().filter(|h| h.len() > 2)?;
            Some((key.clone(), root.join("objects").join(&hash[..2]).join(hash)))
        });
        Some(Layer::Index(objects.collect()))
    }

    fn path(&self, ns: &str, path: &str) -> Option<PathBuf> {
        match self {
            Layer::Namespace { namespace, dir } => (namespace == ns).then(|| dir.join(path)),
            Layer::Index(objects) => objects.get(&format!("{ns}/{path}")).cloned(),
        }
    }
}

fn walk(dir: &Path, prefix: &str, suffix: &str, out: &mut std::collections::BTreeSet<String>) {
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.path().is_dir() {
            walk(&entry.path(), &format!("{prefix}{name}/"), suffix, out);
        } else if name.ends_with(suffix) {
            out.insert(format!("{prefix}{name}"));
        }
    }
}
