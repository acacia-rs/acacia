//! What a Bedrock server sends for the Java render state the oracle set: the Bedrock identifier of
//! a Java kind and the answers to the Molang queries acacia reads. The Bedrock side of each value
//! is stated here once, independent of `kinds::VARIANTS`, so a wrong table entry diverges.

use acacia_render::entity::molang::Value;
use serde_json::Value as Json;

pub fn bedrock_kind(java: &str) -> String {
    let id = java.trim_start_matches("minecraft:");
    let bedrock = match id {
        "tropical_fish" => "tropicalfish",
        "zombified_piglin" => "zombie_pigman",
        "evoker" => "evocation_illager",
        "villager" => "villager_v2",
        "zombie_villager" => "zombie_villager_v2",
        _ => id,
    };
    format!("minecraft:{bedrock}")
}

/// Bedrock's legacy dye index (black first) of a Java dye id (white first).
fn legacy_dye(java: i64) -> i64 {
    15 - java
}

/// A Java enum value's index in Bedrock's `variant` (or `mark_variant`) for `kind`'s `field`.
fn enum_index(kind: &str, field: &str, value: &str) -> Option<i64> {
    let order: &[&str] = match (kind, field) {
        ("fox", "variant") => &["RED", "SNOW"],
        ("llama" | "trader_llama", "variant") => &["CREAMY", "WHITE", "BROWN", "GRAY"],
        ("horse", "variant") => &["WHITE", "CREAMY", "CHESTNUT", "BROWN", "BLACK", "GRAY", "DARK_BROWN"],
        ("horse", "markings") => &["NONE", "WHITE", "WHITE_FIELD", "WHITE_DOTS", "BLACK_DOTS"],
        ("rabbit", "variant") => &["BROWN", "WHITE", "BLACK", "WHITE_SPLOTCHED", "GOLD", "SALT"],
        ("parrot", "variant") => &["RED_BLUE", "BLUE", "GREEN", "YELLOW_BLUE", "GRAY"],
        ("axolotl", "variant") => &["LUCY", "CYAN", "GOLD", "WILD", "BLUE"],
        ("mooshroom", "variant") => &["RED", "BROWN"],
        ("panda", "variant") => &["NORMAL", "LAZY", "WORRIED", "PLAYFUL", "BROWN", "WEAK", "AGGRESSIVE"],
        ("frog", "variant") => &["temperate", "cold", "warm"],
        ("cat", "variant") => &["white", "black", "red", "siamese", "british_shorthair", "calico", "persian", "ragdoll", "tabby", "all_black", "jellie"],
        (_, "profession") => &[
            "none", "farmer", "fisherman", "shepherd", "fletcher", "librarian", "cartographer", "cleric", "armorer", "weaponsmith", "toolsmith",
            "butcher", "leatherworker", "mason", "nitwit",
        ],
        (_, "type") => &["plains", "desert", "jungle", "savanna", "snow", "swamp", "taiga"],
        _ => return None,
    };
    order.iter().position(|v| *v == value).map(|i| i as i64)
}

/// `TropicalFish.Pattern`s, the small shape's six then the large's: Bedrock's `variant` is the
/// shape and `mark_variant` the pattern within it.
const TROPICAL: [&str; 12] =
    ["KOB", "SUNSTREAK", "SNOOPER", "DASHER", "BRINELY", "SPOTTY", "FLOPPER", "STRIPEY", "GLITTER", "BLOCKFISH", "BETTY", "CLAYFISH"];

/// Why a Bedrock server cannot send this state, if it cannot: a Java-only variant, or one
/// carried by data acacia does not track yet.
pub fn unreachable(kind: &str, fields: &Json) -> Option<&'static str> {
    let text = |field: &str| fields[field].as_str().unwrap_or_default().to_owned();
    match kind.trim_start_matches("minecraft:") {
        "wolf" if !matches!(skin(fields).as_str(), "wolf" | "wolf_tame" | "wolf_angry") => Some("wolf variants are actor properties"),
        "rabbit" if text("variant") == "EVIL" => Some("the killer bunny is Java's"),
        "salmon" if text("variant") != "MEDIUM" => Some("Bedrock sizes salmon by scale alone"),
        "copper_golem" if text("weathering") != "UNAFFECTED" => Some("oxidation is an actor property"),
        "cow" | "pig" | "chicken" if !matches!(text("variant").as_str(), "" | "temperate") => Some("climate variants are actor properties"),
        _ => None,
    }
}

/// The file name of the texture the entity chose (`texture`), without `_baby`: `cat_black`.
fn skin(fields: &Json) -> String {
    let path = fields["texture"].as_str().unwrap_or_default();
    let name = path.rsplit('/').next().unwrap_or_default().trim_end_matches(".png");
    name.trim_end_matches("_baby").to_owned()
}

/// The answer to Molang query `name` for a Java `kind` whose state has `fields`.
pub fn query(kind: &str, fields: &Json, name: &str) -> Value {
    let kind = kind.trim_start_matches("minecraft:");
    let flag = |field: &str| fields[field].as_bool().unwrap_or(false);
    let text = |field: &str| fields[field].as_str().unwrap_or_default().to_owned();
    let num = |v: i64| Value::Num(v as f32);
    let collar = fields.get("collarColor").and_then(Json::as_i64);
    match name {
        "is_baby" => num(i64::from(flag("isBaby"))),
        "is_sheared" => num(i64::from(flag("isSheared"))),
        "is_powered" => num(i64::from(flag("isPowered"))),
        "is_angry" => num(i64::from(flag("isAngry"))),
        "has_nectar" => num(i64::from(flag("hasNectar"))),
        "is_sleeping" => num(i64::from(flag("isSleeping"))),
        "is_chested" => num(i64::from(flag("hasChest"))),
        "is_charging" => num(i64::from(flag("isCharging"))),
        "is_sitting" => num(i64::from(flag("isSitting"))),
        "is_shaking" => num(i64::from(flag("isSuffocating"))),
        "is_tamed" => num(i64::from(collar.is_some())),
        "color" => match kind {
            "sheep" => num(fields["woolColor"].as_i64().unwrap_or(0)),
            "tropical_fish" => num(fields["baseColor"].as_i64().unwrap_or(0)),
            _ => num(collar.unwrap_or(0)),
        },
        "color2" if kind == "tropical_fish" => num(fields["patternColor"].as_i64().unwrap_or(0)),
        "trade_tier" => num(fields["villagerData"]["level"].as_i64().unwrap_or(1) - 1),
        // Health in the middle of each `Crackiness.GOLEM` band of 100.
        "health" if kind == "iron_golem" => match text("crackiness").as_str() {
            "LOW" => num(60),
            "MEDIUM" => num(40),
            "HIGH" => num(10),
            _ => num(100),
        },
        "variant" => match kind {
            "villager" | "zombie_villager" => num(enum_index(kind, "profession", fields["villagerData"]["profession"].as_str().unwrap_or("")).unwrap_or(0)),
            "cat" | "frog" => num(enum_index(kind, "variant", skin(fields).trim_start_matches(&format!("{kind}_"))).unwrap_or(0)),
            "pufferfish" => num(fields["puffState"].as_i64().unwrap_or(0)),
            "shulker" => num(fields.get("color").and_then(Json::as_i64).map_or(16, legacy_dye)),
            "tropical_fish" => num(TROPICAL.iter().position(|p| *p == text("pattern")).map_or(0, |i| i as i64 / 6)),
            _ => num(enum_index(kind, "variant", &text("variant")).unwrap_or(0)),
        },
        "mark_variant" => match kind {
            "villager" | "zombie_villager" => num(enum_index(kind, "type", fields["villagerData"]["type"].as_str().unwrap_or("")).unwrap_or(0)),
            "tropical_fish" => num(TROPICAL.iter().position(|p| *p == text("pattern")).map_or(0, |i| i as i64 % 6)),
            _ => num(enum_index(kind, "markings", &text("markings")).unwrap_or(0)),
        },
        "property:minecraft:climate_variant" => Value::Text("temperate".into()),
        "property:minecraft:creaking_state" => Value::Text(if flag("eyesGlowing") { "hostile_observed" } else { "neutral" }.into()),
        _ => num(0),
    }
}
