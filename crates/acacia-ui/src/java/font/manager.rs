//! `FontManager`: every `font/**/*.json` a font (`minecraft:font/include/space.json` is the font
//! `minecraft:include/space`), its providers in order with references expanded (a reference's
//! filter merged over the referenced providers'), each provider loaded once. A font whose
//! definition does not parse has no providers; one with a provider that fails to load or a
//! reference that does not resolve (missing, failed, or a cycle) is left out, and a style naming a
//! font that is not there draws every character as the missing glyph.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use serde_json::Value;

use super::provider::{self, Provider};
use super::resources::Resources;
use super::set::{Conditional, Filter};

enum Entry {
    Load(Result<Arc<Provider>, String>, Filter),
    Reference(String, Filter),
}

/// What loading the fonts said (`LOGGER.warn`s), for tools to show.
pub type Warnings = Vec<String>;

/// Every font's resolved providers, by font id.
pub fn load(resources: &Resources, warnings: &mut Warnings) -> BTreeMap<String, Vec<Conditional>> {
    let mut entries: BTreeMap<String, Vec<Entry>> = BTreeMap::new();
    for file in resources.list("font", ".json") {
        let (ns, path) = super::resources::split_id(&file);
        let id = format!("{ns}:{}", &path["font/".len()..path.len() - ".json".len()]);
        let parsed = resources.read(&file).ok_or("unreadable".to_owned()).and_then(|b| definition(&b, resources));
        let list = parsed.unwrap_or_else(|e| {
            warnings.push(format!("unable to load font {id}: {e}"));
            Vec::new()
        });
        for e in &list {
            if let Entry::Load(Err(why), _) = e {
                warnings.push(format!("failed to load a provider of {id}: {why}"));
            }
        }
        entries.insert(id, list);
    }
    let mut resolved = BTreeMap::new();
    let ids: Vec<String> = entries.keys().cloned().collect();
    for id in ids {
        resolve(&id, &entries, &mut resolved, &mut Vec::new(), warnings);
    }
    resolved.into_iter().filter_map(|(id, r)| Some((id, r?))).collect()
}

/// A definition file's providers; any one that does not decode fails the file.
fn definition(bytes: &[u8], resources: &Resources) -> Result<Vec<Entry>, String> {
    let json: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let list = json["providers"].as_array().ok_or("no providers")?;
    let mut out = Vec::with_capacity(list.len());
    for p in list {
        let filter = Filter::parse(&p["filter"])?;
        out.push(match p["type"].as_str() {
            Some("reference") => Entry::Reference(provider::qualify(p["id"].as_str().ok_or("reference without id")?), filter),
            Some("bitmap" | "space" | "unihex") => {
                check(p)?;
                Entry::Load(provider::load(p, resources).map(Arc::new), filter)
            }
            Some("ttf") => Entry::Load(Err("ttf providers are not loaded (vanilla has none)".into()), filter),
            other => return Err(format!("unknown provider type {other:?}")),
        });
    }
    Ok(out)
}

/// What the codecs refuse at decode time (a failing load only drops the font later).
fn check(p: &Value) -> Result<(), String> {
    if p["type"] == "bitmap" {
        let rows = p["chars"].as_array().ok_or("no chars")?;
        let len = |r: &Value| r.as_str().map(|s| s.chars().count());
        let first = rows.first().and_then(len).filter(|&n| n > 0).ok_or("expected to find data in codepoint grid")?;
        if rows.iter().any(|r| len(r) != Some(first)) {
            return Err("lines in codepoint grid have to be the same length".into());
        }
        let (height, ascent) = (p.get("height").map_or(Some(8), Value::as_i64), p["ascent"].as_i64());
        match (height, ascent) {
            (Some(h), Some(a)) if a <= h => {}
            _ => return Err("bad height or ascent".into()),
        }
    }
    if p["type"] == "unihex" {
        for o in p["size_overrides"].as_array().into_iter().flatten() {
            let cp = |k: &str| o[k].as_str().filter(|s| s.chars().count() == 1).and_then(|s| s.chars().next());
            match (cp("from"), cp("to")) {
                (Some(from), Some(to)) if from < to => {}
                _ => return Err("invalid size override range".into()),
            }
        }
    }
    if p["type"] == "space" && !p["advances"].as_object().is_some_and(|a| a.iter().all(|(k, v)| k.chars().count() == 1 && v.is_number())) {
        return Err("bad advances".into());
    }
    Ok(())
}

fn resolve(id: &str, entries: &BTreeMap<String, Vec<Entry>>, done: &mut BTreeMap<String, Option<Vec<Conditional>>>, visiting: &mut Vec<String>, warnings: &mut Warnings) -> Option<Vec<Conditional>> {
    if let Some(r) = done.get(id) {
        return r.clone();
    }
    let list = entries.get(id)?;
    if visiting.iter().any(|v| v == id) {
        return None;
    }
    visiting.push(id.to_owned());
    let mut out = Some(Vec::new());
    for e in list {
        let part = match e {
            Entry::Load(Ok(p), filter) => Some(vec![Conditional { provider: Arc::clone(p), filter: *filter }]),
            Entry::Load(Err(_), _) => None,
            Entry::Reference(target, filter) => {
                let r = resolve(target, entries, done, visiting, warnings);
                if r.is_none() {
                    warnings.push(format!("can't find font {target} referenced by {id}"));
                }
                r.map(|list| list.into_iter().map(|c| Conditional { filter: filter.merge(&c.filter), ..c }).collect())
            }
        };
        match (part, &mut out) {
            (Some(p), Some(o)) => o.extend(p),
            _ => out = None,
        }
    }
    visiting.pop();
    done.insert(id.to_owned(), out.clone());
    out
}

/// The unihex and bitmap providers by file, for looking texels up.
pub fn by_file(fonts: &BTreeMap<String, Vec<Conditional>>) -> HashMap<Arc<str>, Arc<Provider>> {
    let mut out = HashMap::new();
    for c in fonts.values().flatten() {
        match &*c.provider {
            Provider::Unihex(file, ..) => out.insert(file.clone(), Arc::clone(&c.provider)),
            Provider::Bitmap(sheet, _) => out.insert(sheet.id.clone(), Arc::clone(&c.provider)),
            Provider::Space(..) => None,
        };
    }
    out
}
