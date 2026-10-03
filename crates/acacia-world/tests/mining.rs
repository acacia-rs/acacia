use acacia_world::{BlockRegistry, Material, Mining, Tool, ToolKind, ToolTier};

fn mining(name: &str) -> Mining {
    let r = BlockRegistry::vanilla();
    r.states_of(name).next().unwrap_or_else(|| panic!("{name} missing")).1.mining
}

fn tool(id: &str) -> Option<Tool> {
    Tool::from_identifier(id)
}

#[test]
fn hardness_and_material() {
    let stone = mining("minecraft:stone");
    assert_eq!((stone.hardness, stone.material), (1.5, Material::Pickaxe));
    assert_eq!((mining("minecraft:dirt").hardness, mining("minecraft:dirt").material), (0.5, Material::Shovel));
    assert_eq!(mining("minecraft:obsidian").hardness, 50.0);
    assert_eq!(mining("minecraft:oak_log").material, Material::Axe);
    assert_eq!(mining("minecraft:oak_leaves").material, Material::Leaves);
    assert_eq!(mining("minecraft:web").material, Material::Cobweb);
    assert_eq!(mining("minecraft:white_wool").material, Material::Wool);
    assert!(mining("minecraft:bedrock").is_unbreakable());
    assert!(!mining("minecraft:stone").is_unbreakable());
    // Custom blocks carry no data.
    let custom = BlockRegistry::vanilla().with_custom_blocks(&[acacia_world::CustomBlock { name: "x:y".into(), state_count: 1 }]);
    assert!(custom.states_of("x:y").next().unwrap().1.mining.hardness.is_nan());
}

#[test]
fn harvest_requirements() {
    let stone = mining("minecraft:stone");
    assert!(!stone.can_harvest(None));
    assert!(!stone.can_harvest(tool("minecraft:diamond_shovel")));
    assert!(stone.can_harvest(tool("minecraft:wooden_pickaxe")));
    assert!(stone.can_harvest(tool("minecraft:golden_pickaxe")));

    let iron_ore = mining("minecraft:iron_ore");
    assert!(!iron_ore.can_harvest(tool("minecraft:wooden_pickaxe")));
    assert!(!iron_ore.can_harvest(tool("minecraft:golden_pickaxe")));
    assert!(iron_ore.can_harvest(tool("minecraft:stone_pickaxe")));
    assert!(iron_ore.can_harvest(tool("minecraft:copper_pickaxe")));

    let obsidian = mining("minecraft:obsidian");
    assert!(!obsidian.can_harvest(tool("minecraft:iron_pickaxe")));
    assert!(obsidian.can_harvest(tool("minecraft:diamond_pickaxe")));

    assert!(mining("minecraft:dirt").can_harvest(None));
    assert!(mining("minecraft:web").can_harvest(tool("minecraft:shears")));
    assert!(mining("minecraft:web").can_harvest(tool("minecraft:wooden_sword")));
}

#[test]
fn tool_identifiers() {
    assert_eq!(tool("minecraft:netherite_axe"), Some(Tool { kind: ToolKind::Axe, tier: Some(ToolTier::Netherite) }));
    assert_eq!(tool("minecraft:shears"), Some(Tool { kind: ToolKind::Shears, tier: None }));
    assert_eq!(tool("minecraft:stone"), None);
    assert_eq!(tool("minecraft:iron_ingot"), None);
    assert_eq!(tool("minecraft:golden_apple"), None);
}
