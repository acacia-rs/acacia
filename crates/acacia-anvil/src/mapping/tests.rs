use super::*;

fn name_props(s: JavaState) -> String {
    let j = java();
    let props: Vec<String> = j.properties(s).map(|(k, v)| format!("{k}={v}")).collect();
    format!("{}[{}]", j.block(s).name, props.join(","))
}

#[test]
fn java_table_is_26_3() {
    let j = java();
    assert_eq!(j.state_count(), 35723);
    assert_eq!(j.block(AIR).name, "minecraft:air");
    let stairs = j
        .state(
            "minecraft:oak_stairs",
            &[("facing", "east"), ("half", "top")],
        )
        .unwrap();
    assert_eq!(
        name_props(stairs),
        "minecraft:oak_stairs[facing=east,half=top,shape=straight,waterlogged=false]"
    );
    let wet = j.with(stairs, "waterlogged", "true").unwrap();
    assert_eq!(j.property(wet, "waterlogged"), Some("true"));
    assert_eq!(j.with(wet, "waterlogged", "false"), Some(stairs));
}

#[test]
fn every_geyser_bedrock_state_is_in_the_registry() {
    let reg = BlockRegistry::vanilla();
    let missing: Vec<_> = java()
        .t
        .bedrock
        .iter()
        .filter(|(n, p, _)| reg.find(n, p).is_none())
        .map(|(n, p, _)| format!("{n}[{p}]"))
        .collect();
    assert!(
        missing.is_empty(),
        "{} unknown, e.g. {:?}",
        missing.len(),
        &missing[..missing.len().min(10)]
    );
}

#[test]
fn common_states_map_back() {
    let reg = BlockRegistry::vanilla();
    let map = StateMap::new(reg);
    let check = |name: &str, props: &str, want: &str| {
        let id = reg
            .find(name, props)
            .unwrap_or_else(|| panic!("{name}[{props}] not in registry"));
        assert_eq!(name_props(map.java(id)), want, "{name}[{props}]");
    };
    check("minecraft:stone", "", "minecraft:stone[]");
    check(
        "minecraft:grass_block",
        "",
        "minecraft:grass_block[snowy=false]",
    );
    check(
        "minecraft:water",
        "liquid_depth=0",
        "minecraft:water[level=0]",
    );
    check("minecraft:air", "", "minecraft:air[]");
    let unmapped = map.unmapped.len();
    assert!(
        unmapped < reg.len() / 3,
        "{unmapped} unmapped Bedrock states"
    );
}

#[test]
fn biomes_invert() {
    let b = biomes();
    assert_eq!(b.name(b.java(1)), "minecraft:plains");
    assert_eq!(b.name(b.java(7)), "minecraft:river");
    assert_eq!(b.name(b.java(9)), "minecraft:the_end");
    assert_eq!(b.name(b.java(99_999)), "minecraft:plains");
}
