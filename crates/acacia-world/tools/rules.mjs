// Bedrock-specific block rules layered on top of minecraft-data. Numbers are from Boar
// (opencollab-incubator/Boar, MIT): collision/BedrockCollision.java, data/block/AbstractBoarBlockState.java,
// geyser/mappings/block/GeyserBlockMappings.java (climbable list).

// Keep in sync with BlockFlags in src/registry/state.rs.
export const FLAG = {
  AIR: 1 << 0,
  FULL_CUBE: 1 << 1,
  WATER: 1 << 2,
  LAVA: 1 << 3,
  CLIMBABLE: 1 << 4,
  SCAFFOLDING: 1 << 5,
  COBWEB: 1 << 6,
  POWDER_SNOW: 1 << 7,
  HONEY: 1 << 8,
  SLIME: 1 << 9,
  SOUL_SAND: 1 << 10,
  BED: 1 << 11,
  SWEET_BERRY: 1 << 12,
  BUBBLE_COLUMN: 1 << 13,
  BUBBLE_DRAG: 1 << 14,
  DYNAMIC_SHAPE: 1 << 15,
};

export const FRICTIONS = [0.6, 0.98, 0.8, 0.989];

const NAME_FLAGS = {
  air: FLAG.AIR, cave_air: FLAG.AIR, void_air: FLAG.AIR,
  water: FLAG.WATER, flowing_water: FLAG.WATER, lava: FLAG.LAVA, flowing_lava: FLAG.LAVA,
  ladder: FLAG.CLIMBABLE, vine: FLAG.CLIMBABLE, twisting_vines: FLAG.CLIMBABLE, weeping_vines: FLAG.CLIMBABLE,
  cave_vines: FLAG.CLIMBABLE, cave_vines_body_with_berries: FLAG.CLIMBABLE, cave_vines_head_with_berries: FLAG.CLIMBABLE,
  scaffolding: FLAG.SCAFFOLDING | FLAG.DYNAMIC_SHAPE,
  web: FLAG.COBWEB, powder_snow: FLAG.POWDER_SNOW | FLAG.DYNAMIC_SHAPE,
  honey_block: FLAG.HONEY, slime: FLAG.SLIME, soul_sand: FLAG.SOUL_SAND, bed: FLAG.BED,
  sweet_berry_bush: FLAG.SWEET_BERRY, bubble_column: FLAG.BUBBLE_COLUMN,
  bamboo: FLAG.DYNAMIC_SHAPE, pointed_dripstone: FLAG.DYNAMIC_SHAPE,
};

const FRICTION_BY_NAME = { ice: 1, packed_ice: 1, frosted_ice: 1, slime: 2, honey_block: 2, blue_ice: 3 };

const FULL = [[0, 0, 0, 1, 1, 1]];
const SCAFFOLDING_TOP = [
  [0, 0.875, 0, 1, 1, 1], [0, 0, 0, 0.125, 1, 0.125], [0.875, 0, 0, 1, 1, 0.125],
  [0, 0, 0.875, 0.125, 1, 1], [0.875, 0, 0.875, 1, 1, 1],
];
const SHAPE_OVERRIDES = {
  sea_lantern: FULL, dragon_egg: FULL, sea_pickle: [],
  shelf_mushroom: [], // 26.50 block without a shape source; assumed walk-through like other fungi

  end_portal_frame: [[0, 0, 0, 1, 0.8125, 1]],
  // Collide only in special cases (see DYNAMIC_SHAPE docs); the static shape is what they usually are.
  scaffolding: SCAFFOLDING_TOP, bamboo: [], pointed_dripstone: [], powder_snow: [],
};

export const short = (name) => name.replace(/^minecraft:/, '');

export function blockFlags(name, props) {
  const n = short(name);
  let f = NAME_FLAGS[n] ?? 0;
  if (n === 'bubble_column' && props.drag_down === 1) f |= FLAG.BUBBLE_DRAG;
  return f;
}

export const frictionIndex = (name) => FRICTION_BY_NAME[short(name)] ?? 0;

// Fences and panes carry `minecraft:connection_<side>` since 26.50. Fence: the 1.5-high post stretched
// to each connected edge. Pane/bars: Boar's buildThinBarsShape (post only when isolated).
export function connectedShape(name, props) {
  const n = short(name);
  const fence = /_fence$/.test(n);
  if (!(fence || /(_pane|_bars)$/.test(n)) || props['minecraft:connection_east'] === undefined) return undefined;
  const [N, E, S, W] = ['north', 'east', 'south', 'west'].map((d) => props[`minecraft:connection_${d}`] === 1);
  if (fence) {
    const [a, b, h] = [0.375, 0.625, 1.5];
    return [
      [a, 0, a, b, h, b],
      ...(N ? [[a, 0, 0, b, h, a]] : []), ...(S ? [[a, 0, b, b, h, 1]] : []),
      ...(W ? [[0, 0, a, a, h, b]] : []), ...(E ? [[b, 0, a, 1, h, b]] : []),
    ];
  }
  const [a, b] = [0.4375, 0.5625];
  if (!(N || E || S || W)) return [[a, 0, a, b, 1, b]];
  return [
    ...(N ? [[a, 0, 0, b, 1, 0.5]] : []), ...(S ? [[a, 0, 0.5, b, 1, 1]] : []),
    ...(W ? [[0, 0, a, 0.5, 1, b]] : []), ...(E ? [[0.5, 0, a, 1, 1, b]] : []),
  ];
}

export const shapeOverride = (name) => SHAPE_OVERRIDES[short(name)];

export const isDoor = (name) => /_door$/.test(short(name));

// Boar door shapes keyed by Java facing / open / hinge (BedrockCollision.java).
const T = 0.1825, U = 1 - T;
const DOOR = {
  north: [[0, 0, 0, 1, 1, T]], south: [[0, 0, U, 1, 1, 1]],
  east: [[U, 0, 0, 1, 1, 1]], west: [[0, 0, 0, T, 1, 1]],
};
export function boarDoorShape(facing, open, hingeRight) {
  const closed = !open;
  switch (facing) {
    case 'south': return closed ? DOOR.north : hingeRight ? DOOR.west : DOOR.east;
    case 'west': return closed ? DOOR.east : hingeRight ? DOOR.north : DOOR.south;
    case 'north': return closed ? DOOR.south : hingeRight ? DOOR.east : DOOR.west;
    default: return closed ? DOOR.west : hingeRight ? DOOR.south : DOOR.north;
  }
}

// Stairs carry `minecraft:corner` since 26.50; semantics assumed to match Java's StairsShape.
const HALF = { east: [0.5, 0, 1, 1], west: [0, 0, 0.5, 1], south: [0, 0.5, 1, 1], north: [0, 0, 1, 0.5] };
const CW = { north: 'east', east: 'south', south: 'west', west: 'north' };
const CCW = { east: 'north', south: 'east', west: 'south', north: 'west' };
const WEIRDO = ['east', 'west', 'south', 'north'];
const intersect = (a, b) => [Math.max(a[0], b[0]), Math.max(a[1], b[1]), Math.min(a[2], b[2]), Math.min(a[3], b[3])];
export function stairShape(props) {
  const corner = props['minecraft:corner'];
  if (corner === undefined) return undefined;
  const f = WEIRDO[props.weirdo_direction];
  const [by0, by1, sy0, sy1] = props.upside_down_bit ? [0.5, 1, 0, 0.5] : [0, 0.5, 0.5, 1];
  const side = corner.endsWith('left') ? CCW[f] : CW[f];
  const rects = corner === 'none' ? [HALF[f]]
    : corner.startsWith('outer') ? [intersect(HALF[f], HALF[side])]
    : [HALF[f], intersect(HALF[side], HALF[{ north: 'south', south: 'north', east: 'west', west: 'east' }[f]])];
  return [[0, by0, 0, 1, by1, 1], ...rects.map(([x0, z0, x1, z1]) => [x0, sy0, z0, x1, sy1, z1])];
}

// Blocks new since minecraft-data's newest block data: borrow the shape of a same-shaped block.
export function templateFor(name) {
  const n = short(name);
  if (/_leaves$/.test(n)) return 'oak_leaves';
  if (/_bed$/.test(n)) return 'bed';
  if (n === 'red_shrub') return 'deadbush';
  const oakAliases = {
    poplar_door: 'wooden_door', poplar_trapdoor: 'trapdoor', poplar_button: 'wooden_button',
    poplar_pressure_plate: 'wooden_pressure_plate', poplar_standing_sign: 'standing_sign',
    poplar_wall_sign: 'wall_sign', poplar_fence_gate: 'fence_gate',
  };
  if (oakAliases[n]) return oakAliases[n];
  if (n.includes('poplar')) return n.replace('poplar', 'oak');
  if (/_double_slab$/.test(n)) return 'oak_double_slab';
  if (/_slab$/.test(n)) return 'oak_slab';
  if (/_stairs$/.test(n)) return 'oak_stairs';
  return undefined;
}
