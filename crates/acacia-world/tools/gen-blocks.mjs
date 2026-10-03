// Regenerates crates/acacia-world/data/blocks.bin. See crates/acacia-world/README.md.
// Usage: node crates/acacia-world/tools/gen-blocks.mjs [cacheDir]
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { readNbt, fnv1_64 } from './nbt.mjs';
import * as R from './rules.mjs';
import { miningTable, UNKNOWN_MINING } from './mining.mjs';

const GEYSER_PALETTE = 'block_palette.26_50.nbt';
const MCDATA_VERSION = '1.26.30';
const SOURCES = {
  [GEYSER_PALETTE]: `https://raw.githubusercontent.com/GeyserMC/Geyser/master/core/src/main/resources/bedrock/${GEYSER_PALETTE}`,
  'blockStates.json': `https://raw.githubusercontent.com/PrismarineJS/minecraft-data/master/data/bedrock/${MCDATA_VERSION}/blockStates.json`,
  'blocks.json': `https://raw.githubusercontent.com/PrismarineJS/minecraft-data/master/data/bedrock/${MCDATA_VERSION}/blocks.json`,
  'blockCollisionShapes.json': `https://raw.githubusercontent.com/PrismarineJS/minecraft-data/master/data/bedrock/${MCDATA_VERSION}/blockCollisionShapes.json`,
  'blocksJ2B.json': `https://raw.githubusercontent.com/PrismarineJS/minecraft-data/master/data/bedrock/${MCDATA_VERSION}/blocksJ2B.json`,
};

const here = dirname(fileURLToPath(import.meta.url));
const cache = process.argv[2] ?? join(here, '.cache');
const out = join(here, '..', 'data', 'blocks.bin');

async function load(file) {
  const p = join(cache, file);
  if (!existsSync(p)) {
    mkdirSync(cache, { recursive: true });
    const res = await fetch(SOURCES[file]);
    if (!res.ok) throw new Error(`${file}: HTTP ${res.status}`);
    writeFileSync(p, Buffer.from(await res.arrayBuffer()));
  }
  return readFileSync(p);
}

const propsOf = (states) => Object.fromEntries(Object.entries(states).map(([k, v]) => [k, typeof v === 'object' ? v.value : v]));
const propsKey = (props) => Object.keys(props).sort().map((k) => `${k}=${props[k]}`).join(',');

const palette = readNbt(await load(GEYSER_PALETTE)).blocks.map((b) => ({ name: b.name, props: propsOf(b.states), hash: b.network_id >>> 0 }));
const mcStates = JSON.parse(await load('blockStates.json'));
const mcBlocks = JSON.parse(await load('blocks.json'));
const shapes = JSON.parse(await load('blockCollisionShapes.json'));
const j2b = JSON.parse(await load('blocksJ2B.json'));

// Runtime ids are palette indices, which requires the FNV-1 64 name order the client uses.
for (let i = 1; i < palette.length; i++) {
  if (palette[i].name !== palette[i - 1].name && fnv1_64(palette[i].name) < fnv1_64(palette[i - 1].name)) {
    throw new Error(`palette not in FNV-1 64 order at ${i}`);
  }
}

const shapeByState = new Map(); // "short|propsKey" -> boxes
const shapeByBlock = new Map(); // short -> boxes, when every state shares one shape
for (const b of mcBlocks) {
  const ids = shapes.blocks[b.name];
  if (!ids) continue;
  if (new Set(ids).size === 1) shapeByBlock.set(b.name, shapes.shapes[ids[0]]);
  for (let i = 0; i <= b.maxStateId - b.minStateId; i++) {
    const st = mcStates[b.minStateId + i];
    shapeByState.set(`${b.name}|${propsKey(propsOf(st.states))}`, shapes.shapes[ids.length === 1 ? ids[0] : ids[i]]);
  }
}

// mcdata gives every door state one shape; rebuild from Boar's Java-keyed rules via the J2B mapping.
const parseState = (s) => {
  const m = /^minecraft:([^[]+)(?:\[(.*)\])?$/.exec(s);
  const props = Object.fromEntries((m[2] ?? '').split(',').filter(Boolean).map((kv) => kv.split('=')));
  return { name: m[1], props };
};
const j2bBool = (v) => (v === 'true' ? 1 : v === 'false' ? 0 : /^\d+$/.test(v) ? Number(v) : v);
for (const [java, bedrock] of Object.entries(j2b)) {
  const j = parseState(java);
  const b = parseState(bedrock);
  if (!R.isDoor(`minecraft:${b.name}`) || !j.props.facing) continue;
  const props = Object.fromEntries(Object.entries(b.props).map(([k, v]) => [k, j2bBool(v)]));
  shapeByState.set(`${b.name}|${propsKey(props)}`, R.boarDoorShape(j.props.facing, j.props.open === 'true', j.props.hinge === 'right'));
}

const unresolved = new Set();
function shapeFor({ name, props }) {
  const override = R.shapeOverride(name);
  if (override) return override;
  const computed = R.stairShape(props) ?? R.connectedShape(name, props);
  if (computed) return computed;
  const n = R.short(name);
  const key = propsKey(props);
  const direct = shapeByState.get(`${n}|${key}`) ?? shapeByBlock.get(n);
  if (direct) return direct;
  const t = R.templateFor(name);
  const templ = t && (shapeByState.get(`${t}|${key}`) ?? shapeByBlock.get(t));
  if (templ) return templ;
  unresolved.add(n);
  return [[0, 0, 0, 1, 1, 1]];
}

const strings = new Map();
const str = (s) => { if (!strings.has(s)) strings.set(s, strings.size); return strings.get(s); };
const shapeIds = new Map();
const shapeList = [];
const shp = (boxes) => {
  const k = JSON.stringify(boxes);
  if (!shapeIds.has(k)) { shapeIds.set(k, shapeList.length); shapeList.push(boxes); }
  return shapeIds.get(k);
};

const mining = miningTable(mcBlocks);
const noMining = new Set();
const miningIds = new Map();
const miningList = [];
function miningFor(name) {
  const n = R.short(name);
  const t = R.templateFor(name);
  const entry = mining.get(n) ?? (t && mining.get(t));
  if (!entry) noMining.add(n);
  const e = entry ?? UNKNOWN_MINING;
  const k = e.join();
  if (!miningIds.has(k)) { miningIds.set(k, miningList.length); miningList.push(e); }
  return miningIds.get(k);
}

const isFull = (b) => b.length === 1 && b[0].join() === '0,0,0,1,1,1';
const states = palette.map((s) => {
  const boxes = shapeFor(s);
  const flags = R.blockFlags(s.name, s.props) | (isFull(boxes) ? R.FLAG.FULL_CUBE : 0);
  const liquid = flags & (R.FLAG.WATER | R.FLAG.LAVA) ? s.props.liquid_depth ?? 0 : 0;
  return [str(s.name), str(propsKey(s.props)), shp(boxes), flags, liquid, R.frictionIndex(s.name), s.hash, miningFor(s.name)];
});

// Layout (little endian), parsed by src/registry/blob.rs:
// "BWB2", u8 n + n*f32 frictions, u32 n + n*(u16 len, utf8) strings, u32 n + n*(u8 boxes, boxes*6 f32) shapes,
// u16 n + n*(f32 hardness, u8 material, u8 harvest tool kinds, u8 min harvest level) mining,
// u32 n + n*(u16 name, u16 props, u16 shape, u16 flags, u8 liquid_depth, u8 friction index,
// u32 network hash = FNV-1a 32 of LE NBT {name, states}, u16 mining) states.
const parts = [Buffer.from('BWB2')];
const u8 = (v) => parts.push(Buffer.from([v]));
const u16 = (v) => { const b = Buffer.alloc(2); b.writeUInt16LE(v); parts.push(b); };
const u32 = (v) => { const b = Buffer.alloc(4); b.writeUInt32LE(v); parts.push(b); };
const f32 = (v) => { const b = Buffer.alloc(4); b.writeFloatLE(v); parts.push(b); };
u8(R.FRICTIONS.length); R.FRICTIONS.forEach(f32);
u32(strings.size);
for (const s of strings.keys()) { const b = Buffer.from(s, 'utf8'); u16(b.length); parts.push(b); }
u32(shapeList.length);
for (const boxes of shapeList) { u8(boxes.length); boxes.flat().forEach(f32); }
u16(miningList.length);
for (const [hardness, mat, kinds, level] of miningList) { f32(hardness); u8(mat); u8(kinds); u8(level); }
u32(states.length);
for (const [n, p, sh, fl, lq, fr, h, mi] of states) { u16(n); u16(p); u16(sh); u16(fl); u8(lq); u8(fr); u32(h); u16(mi); }

mkdirSync(dirname(out), { recursive: true });
const blob = Buffer.concat(parts);
writeFileSync(out, blob);
const air = palette.findIndex((s) => s.name === 'minecraft:air');
console.log(`${states.length} states, ${strings.size} strings, ${shapeList.length} shapes, air=${air}, ${blob.length} bytes`);
if (unresolved.size) console.log(`full-cube fallback (no shape source): ${[...unresolved].join(' ')}`);
if (noMining.size) console.log(`unknown hardness (no mining source): ${[...noMining].join(' ')}`);
