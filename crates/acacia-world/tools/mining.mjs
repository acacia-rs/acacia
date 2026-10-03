// Block-breaking data from minecraft-data bedrock blocks.json (hardness, material, harvestTools).
// Keep MATERIAL, TOOL_KIND and the tier order in sync with src/registry/mining.rs.

export const MATERIAL = {
  DEFAULT: 0, PICKAXE: 1, SHOVEL: 2, AXE: 3, HOE: 4, LEAVES: 5, PLANT: 6, GOURD: 7, VINE: 8,
  COBWEB: 9, WOOL: 10, SWORD_INSTANT: 11,
};
const TOOL_KIND = { SWORD: 0, SHOVEL: 1, PICKAXE: 2, AXE: 3, HOE: 4, SHEARS: 5 };
// Harvest level per tier, tiers in minecraft-data's id order: wood, copper, stone, gold, iron, diamond, netherite.
const TIER_LEVEL = [0, 1, 1, 0, 2, 3, 4];

// minecraft-data harvestTools ids: 939 + 5 * tier + kind (sword, shovel, pickaxe, axe, hoe); 1134 = shears.
const FIRST_TOOL_ID = 939;
const SHEARS_ID = 1134;

function material(m) {
  if (m === undefined || m === 'default') return MATERIAL.DEFAULT;
  if (m.startsWith('incorrect_for_') || m === 'mineable/pickaxe') return MATERIAL.PICKAXE;
  if (m === 'mineable/shovel') return MATERIAL.SHOVEL;
  if (m === 'mineable/axe') return MATERIAL.AXE;
  if (m === 'mineable/hoe') return MATERIAL.HOE;
  if (m.startsWith('leaves;')) return MATERIAL.LEAVES;
  if (m.startsWith('plant;')) return MATERIAL.PLANT;
  if (m.startsWith('gourd;')) return MATERIAL.GOURD;
  if (m.startsWith('vine_or_glow_lichen;')) return MATERIAL.VINE;
  if (m === 'coweb') return MATERIAL.COBWEB;
  if (m === 'wool') return MATERIAL.WOOL;
  if (m === 'sword_instantly_mines') return MATERIAL.SWORD_INSTANT;
  throw new Error(`unknown minecraft-data material ${m}`);
}

/** [kinds bitmask (0 = anything harvests), minimum harvest level]. */
function harvest(tools) {
  const ids = Object.keys(tools ?? {}).map(Number);
  if (!ids.length) return [0, 0];
  let kinds = 0;
  let level = 255;
  for (const id of ids) {
    if (id === SHEARS_ID) { kinds |= 1 << TOOL_KIND.SHEARS; level = Math.min(level, 0); continue; }
    const off = id - FIRST_TOOL_ID;
    if (off < 0 || off >= 35) throw new Error(`unknown harvest tool id ${id}`);
    kinds |= 1 << (off % 5);
    level = Math.min(level, TIER_LEVEL[Math.floor(off / 5)]);
  }
  return [kinds, level];
}

/** name (without namespace) -> [hardness, material, harvest kinds, min level]; NaN hardness = unknown. */
export function miningTable(mcBlocks) {
  const byName = new Map();
  for (const b of mcBlocks) byName.set(b.name, [b.hardness ?? NaN, material(b.material), ...harvest(b.harvestTools)]);
  return byName;
}

export const UNKNOWN_MINING = [NaN, MATERIAL.DEFAULT, 0, 0];
