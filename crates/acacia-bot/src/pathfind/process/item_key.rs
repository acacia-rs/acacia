//! Item keys: an identifier, or an identifier with the aux value a stack must have (`minecraft:bed@14`, a red
//! bed). Acts, restocks and inventory counts take keys; a plain identifier matches any aux.

/// The identifier and the aux value, if the key names one.
pub fn split(key: &str) -> (&str, Option<u32>) {
    key.rsplit_once('@').and_then(|(name, aux)| Some((name, Some(aux.parse().ok()?)))).unwrap_or((key, None))
}

pub fn join(name: &str, aux: Option<u32>) -> String {
    match aux {
        Some(a) => format!("{name}@{a}"),
        None => name.to_string(),
    }
}

/// A stack named `name` with aux `metadata` is what `key` asks for.
pub fn fits(key: &str, name: Option<&str>, metadata: u32) -> bool {
    let (want, aux) = split(key);
    name == Some(want) && aux.is_none_or(|a| a == metadata)
}

#[cfg(test)]
#[test]
fn keys_carry_an_aux_value() {
    assert_eq!(split("minecraft:bed@14"), ("minecraft:bed", Some(14)));
    assert_eq!(split("minecraft:bed"), ("minecraft:bed", None));
    assert_eq!(join("minecraft:bed", Some(3)), "minecraft:bed@3");
    assert!(fits("minecraft:bed", Some("minecraft:bed"), 5));
    assert!(fits("minecraft:bed@5", Some("minecraft:bed"), 5));
    assert!(!fits("minecraft:bed@5", Some("minecraft:bed"), 14));
}
