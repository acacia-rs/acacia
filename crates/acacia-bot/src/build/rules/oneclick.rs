//! Blocks one click decides beyond the B1 set, measured with `place_probe SET=oneclick` (2026-10-28, BDS
//! 1.26.52.3; docs/builder/oneclick.md). Hooked from `rules/mod.rs`.

use super::Dir;

/// What a click gives a placement: the clicked face, the horizontal look, and the six-way direction towards the
/// player (`rules::six_way`).
pub(super) struct Click {
    pub dir: Dir,
    pub look: Option<Dir>,
    pub toward: Option<Dir>,
}

impl Dir {
    /// A quarter turn clockwise seen from above (horizontal directions only).
    fn clockwise(self) -> Dir {
        match self {
            Dir::North => Dir::East,
            Dir::East => Dir::South,
            Dir::South => Dir::West,
            Dir::West => Dir::North,
            d => d,
        }
    }

    /// As `direction` (bells, looms) names it: south 0, west 1, north 2, east 3.
    fn quarter(self) -> &'static str {
        match self {
            Dir::South => "0",
            Dir::West => "1",
            Dir::North => "2",
            _ => "3",
        }
    }
}

/// `direction` blocks that face the player.
const TOWARDS: &[&str] = &["loom", "beehive", "bee_nest", "decorated_pot", "chiseled_bookshelf"];

/// Name endings this tier places although `DENIED`'s substrings match them by accident ("door", "fire",
/// "redstone").
const CLAIMS: &[&str] = &["trapdoor", "campfire", "redstone_block", "redstone_lamp", "redstone_torch", "redstone_ore"];

pub(super) fn claims(n: &str) -> bool {
    CLAIMS.iter().any(|c| n.ends_with(c)) || (n.starts_with("dead_") && n.ends_with("_coral_block"))
}

/// Set by the server from neighbours, power or time: never compared. Leaves: a placed leaf is persistent, a
/// structure's natural one not; the look is the same.
pub(super) fn derived(k: &str) -> bool {
    matches!(k, "in_wall_bit" | "redstone_signal" | "toggle_bit" | "update_bit" | "persistent_bit")
}

/// Values shown alike: a wall bell is `side` or `multiple` by whether the cell across from its wall is filled.
pub(super) fn alike(k: &str, v: &str, w: &str) -> bool {
    k == "attachment" && [v, w].iter().all(|a| matches!(*a, "side" | "multiple"))
}

/// The value every placement gives (anything else takes more than a click).
fn fixed(k: &str) -> Option<&'static str> {
    match k {
        "open_bit" | "deprecated" | "extinguished" | "honey_level" | "composter_fill_level" | "books_stored" | "explode_bit" | "fill_level" | "lit" => Some("0"),
        "cauldron_liquid" => Some("water"),
        _ => None,
    }
}

/// The target's fixed states are the ones a placement gives.
pub(super) fn fixed_ok(k: &str, v: &str) -> bool {
    fixed(k).is_none_or(|f| f == v)
}

/// Lit and unlit twins whose light follows power: placed as, and compared as, the base block.
pub(super) fn base(name: &str) -> &str {
    let n = name.trim_start_matches("minecraft:");
    match n {
        "unlit_redstone_torch" => "minecraft:redstone_torch",
        "lit_redstone_lamp" => "minecraft:redstone_lamp",
        "lit_redstone_ore" => "minecraft:redstone_ore",
        "lit_deepslate_redstone_ore" => "minecraft:deepslate_redstone_ore",
        _ => name,
    }
}

/// Falls when the cell under it is open.
pub(super) fn gravity(n: &str) -> bool {
    n.ends_with("anvil")
}

/// Hangs from the block above (a click on its Down face); see `rules::hangs`.
pub(super) fn hangs(name: &str, props: &str) -> Option<bool> {
    let has = |kv: &str| props.split(',').any(|p| p.trim_start_matches("minecraft:") == kv);
    let rod = name.ends_with("end_rod") || name.ends_with("lightning_rod");
    let only = (name.ends_with("lantern") && has("hanging=1"))
        || ((name.ends_with("bell") || name.ends_with("grindstone")) && has("attachment=hanging"))
        || (rod && has("facing_direction=0"));
    match () {
        _ if only => Some(true),
        _ if name.ends_with("_chain") && has("pillar_axis=y") => Some(false),
        _ => None,
    }
}

/// A y chain's ends take a click at their centre (chandeliers: a chain under a chain, a lantern under it).
pub(super) fn post(name: &str, props: &str) -> bool {
    name.ends_with("_chain") && props.split(',').any(|p| p.trim_start_matches("minecraft:") == "pillar_axis=y")
}

/// This tier decides `k` of `name` (else `rules` does).
pub(super) fn controls(name: &str, k: &str) -> bool {
    fixed(k).is_some() || decide(name, k, &Click { dir: Dir::Up, look: None, toward: None }, "").is_some()
}

/// The value the click gives `k` of `name` (`v` is the target's, for its encoding): `None` if this tier does not
/// decide `k`, `Some(None)` if no value is certain from this click.
pub(super) fn decide(name: &str, k: &str, c: &Click, v: &str) -> Option<Option<&'static str>> {
    let n = name.trim_start_matches("minecraft:");
    if let Some(f) = fixed(k) {
        return Some(Some(f));
    }
    Some(match k {
        "direction" if n.ends_with("trapdoor") => c.look.map(|d| d.opposite().weirdo()),
        "cardinal_direction" if n.ends_with("campfire") => c.look.map(Dir::word),
        "cardinal_direction" if n.ends_with("anvil") => c.look.map(|d| d.opposite().clockwise().word()),
        "facing_direction" if n == "hopper" => Some(match c.dir {
            Dir::Up | Dir::Down => Dir::Down.int(),
            d => d.opposite().int(),
        }),
        // BDS names an end rod's horizontal facings backwards (a north-pointing rod reads 3).
        "facing_direction" if n == "end_rod" => Some(match c.dir {
            d @ (Dir::Up | Dir::Down) => d.int(),
            d => d.opposite().int(),
        }),
        "facing_direction" if n.ends_with("lightning_rod") => Some(c.dir.int()),
        "facing_direction" if n == "barrel" => c.toward.map(Dir::int),
        "facing_direction" if n.ends_with("glazed_terracotta") => c.look.map(|d| d.opposite().int()),
        "attachment" if n == "bell" || n == "grindstone" => Some(match c.dir {
            Dir::Up => "standing",
            Dir::Down => "hanging",
            _ if v == "multiple" => "multiple",
            _ => "side",
        }),
        "direction" if n == "bell" || n == "grindstone" => match c.dir {
            Dir::Up | Dir::Down => c.look.map(|d| d.opposite().quarter()),
            d => Some(d.quarter()),
        },
        "direction" if TOWARDS.contains(&n) => c.look.map(|d| d.opposite().quarter()),
        "block_face" if n.starts_with("amethyst_cluster") || n.ends_with("amethyst_bud") => Some(c.dir.word()),
        "hanging" if n.ends_with("lantern") => match c.dir {
            Dir::Down => Some("1"),
            _ => Some("0"),
        },
        _ => return None,
    })
}
