// Light emission and filtering per state: minecraft-data bedrock blocks.json `emitLight`/`filterLight`,
// corrected from minecraft.wiki/w/Light (its Bedrock filter table and the emission table).
import { short, templateFor } from './rules.mjs';

const EMIT = {
  lit_furnace: 13, lit_blast_furnace: 13, lit_smoker: 13,
  lit_redstone_lamp: 15, lit_redstone_ore: 9, lit_deepslate_redstone_ore: 9, unlit_redstone_torch: 0,
  underwater_torch: 14, colored_torch_blue: 14, colored_torch_green: 14, colored_torch_purple: 14, colored_torch_red: 14,
  glowingobsidian: 12, glow_lichen: 7, cave_vines_body_with_berries: 14, cave_vines_head_with_berries: 14,
};
const FILTER = {
  beacon: 14, anvil: 3, chipped_anvil: 3, damaged_anvil: 3, deprecated_anvil: 3,
  hopper: 2, brewing_stand: 2, cauldron: 2, ice: 2, frosted_ice: 2, water: 1, flowing_water: 1, web: 1, powder_snow: 1,
};
// Opaque full cubes that minecraft-data marks clear (legacy and Education Edition blocks).
const OPAQUE = new Set([
  'glowingobsidian', 'underwater_tnt', 'info_update', 'info_update2', 'reserved6', 'netherreactor', 'stonecutter',
  'deprecated_purpur_block_1', 'deprecated_purpur_block_2', 'allow', 'deny', 'camera', 'chemical_heat',
  'compound_creator', 'material_reducer', 'element_constructor', 'lab_table',
]);
const BULB = { copper_bulb: 15, exposed_copper_bulb: 12, weathered_copper_bulb: 8, oxidized_copper_bulb: 4 };
const ANCHOR = [0, 3, 7, 11, 15];
const TRIAL_SPAWNER = [4, 4, 9, 9, 9, 4];

function emission(n, props, base) {
  if (n in EMIT) return EMIT[n];
  const bulb = BULB[n.replace(/^waxed_/, '')];
  if (bulb !== undefined) return props.lit ? bulb : 0;
  const light = /^light_block_(\d+)$/.exec(n);
  if (light) return Number(light[1]);
  if (/(^|_)candle$/.test(n)) return props.lit ? 3 * (props.candles + 1) : 0;
  if (/(^|_)candle_cake$/.test(n)) return props.lit ? 3 : 0;
  if (n === 'sea_pickle') return props.dead_bit ? 0 : 6 + 3 * props.cluster_count;
  if (n === 'respawn_anchor') return ANCHOR[props.respawn_anchor_charge];
  if (n === 'campfire' || n === 'soul_campfire') return props.extinguished ? 0 : base;
  if (n === 'cauldron') return props.cauldron_liquid === 'lava' && props.fill_level > 0 ? 15 : 0;
  if (n === 'vault') return props.vault_state === 'inactive' ? 6 : 12;
  if (n === 'trial_spawner') return TRIAL_SPAWNER[props.trial_spawner_state];
  return base;
}

function filter(n, base) {
  if (n in FILTER) return FILTER[n];
  if (OPAQUE.has(n) || /^element_\d+$/.test(n)) return 15;
  if (/_leaves(_flowered)?$/.test(n)) return 2;
  if (/_slab$/.test(n)) return /(^|_)double_/.test(n) ? 15 : 1;
  return base;
}

/** [emission, filter], each 0..=15. Blocks unknown to minecraft-data emit nothing and block all light. */
export function lightFor(mcByName, name, props) {
  const n = short(name);
  const b = mcByName.get(n) ?? mcByName.get(templateFor(name));
  return [emission(n, props, b?.emitLight ?? 0), filter(n, b?.filterLight ?? 15)];
}
