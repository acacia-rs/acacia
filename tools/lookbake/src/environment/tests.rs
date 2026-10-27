use acacia_render::sky::{Attr, Environment};

use super::*;

fn put(dir: &Path, name: &str, doc: Value) {
    let path = dir.join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, serde_json::to_vec(&doc).unwrap()).unwrap();
}

fn data(dir: &Path) {
    put(dir, "timeline/day.json", json!({
        "clock": "minecraft:overworld",
        "period_ticks": 24000,
        "tracks": {
            "minecraft:visual/sky_color": {"keyframes": [{"ticks": 0, "value": "#ffffff"}, {"ticks": 12000, "value": "#000000"}], "modifier": "multiply"},
            "minecraft:gameplay/monsters_burn": {"keyframes": [{"ticks": 0, "value": true}]}
        }
    }));
    put(dir, "timeline/villager_schedule.json", json!({"period_ticks": 24000, "tracks": {"minecraft:gameplay/villager_activity": {"keyframes": []}}}));
    put(dir, "tags/timeline/universal.json", json!({"values": ["minecraft:villager_schedule"]}));
    put(dir, "tags/timeline/in_overworld.json", json!({"values": ["#minecraft:universal", "minecraft:day"]}));
    put(dir, "dimension_type/overworld.json", json!({
        "attributes": {"minecraft:visual/sky_color": "#78a7ff", "minecraft:audio/background_music": {}},
        "has_skylight": true, "has_ceiling": false, "timelines": "#minecraft:in_overworld"
    }));
    put(dir, "dimension_type/the_nether.json", json!({
        "attributes": {"minecraft:gameplay/sky_light_level": 4.0}, "has_skylight": false, "has_ceiling": true, "skybox": "none", "timelines": "#minecraft:universal"
    }));
    put(dir, "worldgen/biome/swamp.json", json!({"attributes": {"minecraft:visual/water_fog_end_distance": {"argument": 0.85, "modifier": "multiply"}}}));
    put(dir, "worldgen/biome/badlands.json", json!({"attributes": {"minecraft:audio/background_music": {}}}));
    put(dir, "worldgen/biome/plains.json", json!({"temperature": 0.8}));
}

#[test]
fn environment_json_keeps_what_the_renderer_reads() {
    let dir = std::env::temp_dir().join(format!("lookbake-environment-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (src, out) = (dir.join("data"), dir.join("out"));
    data(&src);
    std::fs::create_dir_all(&out).unwrap();
    write(&src, &out).unwrap();
    let doc: Value = serde_json::from_slice(&std::fs::read(out.join("environment.json")).unwrap()).unwrap();
    assert_eq!(doc["dimensions"]["overworld"]["timelines"], json!(["day"]), "tags expanded, timelines with nothing rendered dropped");
    assert!(doc["dimensions"]["overworld"]["attributes"].get("minecraft:audio/background_music").is_none());
    assert!(doc["timelines"]["day"]["tracks"].get("minecraft:gameplay/monsters_burn").is_none());
    // Badlands has a map of its own (no rendered attributes); plains shares the empty one.
    assert!(doc["biomes"]["badlands"]["attributes"].as_object().unwrap().is_empty() && doc["biomes"].get("plains").is_none());

    let env = Environment::parse(&doc);
    let overworld = env.dimension("overworld").unwrap();
    let dusk = env.sample(overworld, 6000, &[], 0.0, 0.0, false);
    assert!((dusk.rgb(Attr::SkyColor)[2] - 0.5).abs() < 1e-6);
    let swamp = env.sample(overworld, 0, &[("swampland", 1.0)], 0.0, 0.0, false);
    assert!((swamp.number(Attr::WaterFogEnd) - 96.0 * 0.85).abs() < 1e-4);
    let nether = env.dimension("the_nether").unwrap();
    assert_eq!(env.sample(nether, 0, &[], 1.0, 0.0, false).darken(), 11.0);
    std::fs::remove_dir_all(&dir).unwrap();
}
