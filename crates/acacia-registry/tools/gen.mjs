// Generates acacia-registry's src/generated/*.rs and acacia-world's data/neutral.bin.
// Usage: node crates/acacia-registry/tools/gen.mjs <cacheDir>   (README.md "Data sources" lists the inputs)
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { readNbt } from '../../acacia-world/tools/nbt.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const CACHE = process.argv[2] ?? path.join(HERE, '.cache');
const GEN = path.join(HERE, '..', 'src', 'generated');
const NEUTRAL_BIN = path.join(HERE, '..', '..', 'acacia-world', 'data', 'neutral.bin');
// Bedrock-only names that collide with a different Java block.
const EXTRA_RENAMES = { 'minecraft:stonecutter': 'minecraft:legacy_stonecutter' };

const read = (f) => fs.readFileSync(path.join(CACHE, f));
const report = JSON.parse(read('java-26.3-blocks.json'));
const dump = JSON.parse(read('java-26.3-block-dump.json'));
const geyser = readNbt(read('geyser-blocks.nbt')).bedrock_mappings;
const bedrock = read('bedrock-states.tsv').toString().trim().split('\n').map((l) => {
  const [name, props] = l.split('\t');
  return { name, props: props ?? '' };
});

const camel = (s) => s.replace(/^minecraft:/, '').split(/[_:.\s]+/).filter(Boolean)
  .map((w) => w[0].toUpperCase() + w.slice(1).toLowerCase()).join('');
const fail = (msg) => { throw new Error(msg); };

// ---- properties ----
// One entry per distinct property: Java's BlockStateProperties constant, or (key, values) for Bedrock extras.
const props = [];
const propByConst = new Map();
const valueTypes = new Map(); // Java value class -> ordered union of serialized values
const addProp = (p) => { p.id = props.length; props.push(p); return p; };
for (const b of dump.blocks) {
  for (const p of b.props) {
    const prev = propByConst.get(p.const);
    if (prev) {
      if (prev.values.join() !== p.values.join()) fail(`${p.const}: value sets differ`);
      continue;
    }
    let type;
    if (p.type === 'Boolean') type = { kind: 'bool' };
    else if (p.type === 'Integer') type = { kind: 'int' };
    else {
      type = { kind: 'enum', name: p.type };
      const u = valueTypes.get(p.type) ?? valueTypes.set(p.type, []).get(p.type);
      for (const v of p.values) if (!u.includes(v)) u.push(v);
    }
    propByConst.set(p.const, addProp({ marker: camel(p.const), key: p.name, values: p.values, type }));
  }
}

// ---- Java blocks: layout from the report (state order = product of properties, last fastest) ----
const stride = (states, k) => { let n = 1; while (n < states.length && states[n].properties[k] === states[0].properties[k]) n++; return n; };
const blocks = dump.blocks.map((b) => {
  const r = report[b.name] ?? fail(`${b.name} missing from report`);
  const keys = Object.keys(r.properties ?? {}).sort((x, y) => stride(r.states, y) - stride(r.states, x));
  const byKey = new Map(b.props.map((p) => [p.name, propByConst.get(p.const)]));
  const layout = keys.map((k) => {
    const p = byKey.get(k) ?? fail(`${b.name}.${k}: no constant`);
    if (p.values.join() !== r.properties[k].join()) fail(`${b.name}.${k}: report order differs`);
    return p;
  });
  if (r.states[0].id !== b.first) fail(`${b.name}: first state`);
  return { name: b.name, first: b.first, count: r.states.length, def: b.default - b.first, layout, java: true };
});
const javaStateCount = blocks.reduce((n, b) => n + b.count, 0);
if (javaStateCount !== geyser.length) fail(`${javaStateCount} Java states, ${geyser.length} in Geyser`);
const javaCombo = (id) => {
  const b = blocks.find((x) => id >= x.first && id < x.first + x.count);
  let rest = id - b.first;
  const combo = [];
  for (let i = b.layout.length - 1; i >= 0; i--) { combo.unshift(rest % b.layout[i].values.length); rest = Math.floor(rest / b.layout[i].values.length); }
  return { block: b, combo };
};

// ---- Bedrock join (Geyser: Java state -> Bedrock state) ----
const bedrockKey = (name, st) => `${name}|${Object.keys(st).sort().map((k) => `${k}=${st[k]}`).join(',')}`;
const bedrockIdByKey = new Map(bedrock.map((s, i) => [`${s.name}|${s.props}`, i]));
const javaToBedrock = geyser.map((entry, id) => {
  const name = `minecraft:${entry.bedrock_identifier ?? javaCombo(id).block.name.slice('minecraft:'.length)}`;
  return bedrockIdByKey.get(bedrockKey(name, entry.state ?? {})) ?? fail(`state ${id}: ${name} not in acacia-world`);
});
const mappedNames = new Set(javaToBedrock.map((b) => bedrock[b].name));
const javaNames = new Set(blocks.map((b) => b.name));

// ---- Bedrock extras: blocks no Java state maps to, appended after the Java states ----
const bedrockPropKey = new Map();
const extraNames = [...new Set(bedrock.map((s) => s.name))].filter((n) => !mappedNames.has(n)).sort();
let nextState = javaStateCount;
for (const bedrockName of extraNames) {
  const states = bedrock.map((s, i) => ({ ...s, i })).filter((s) => s.name === bedrockName);
  const values = new Map();
  for (const s of states) for (const kv of s.props.split(',').filter(Boolean)) {
    const [k, v] = kv.split('=');
    (values.get(k) ?? values.set(k, new Set()).get(k)).add(v);
  }
  const layout = [...values.keys()].sort().map((k) => {
    const vs = [...values.get(k)];
    const numeric = vs.every((v) => /^\d+$/.test(v));
    const isBool = k.endsWith('_bit') && vs.every((v) => v === '0' || v === '1');
    const ordered = isBool ? ['1', '0'] : numeric ? vs.map(Number).sort((a, b) => a - b).map(String) : vs.sort();
    const id = `${k}|${ordered.join(',')}`;
    if (!bedrockPropKey.has(id)) {
      const base = `Bedrock${camel(k)}`;
      const taken = props.some((p) => p.marker === base);
      bedrockPropKey.set(id, addProp({
        marker: taken ? `${base}${ordered.length}` : base, key: k, values: ordered, bedrockValues: ordered,
        type: isBool ? { kind: 'bool' } : numeric ? { kind: 'int' } : { kind: 'str' },
      }));
    }
    return bedrockPropKey.get(id);
  });
  const count = layout.reduce((n, p) => n * p.values.length, 1);
  if (count !== states.length) fail(`${bedrockName}: ${states.length} states, not a full product`);
  const name = EXTRA_RENAMES[bedrockName] ?? bedrockName;
  if (javaNames.has(name)) fail(`${name}: Bedrock-only name collides with a Java block`);
  const block = { name, bedrockName, first: nextState, count, def: 0, layout, java: false, bedrockIds: [] };
  for (const s of states) {
    const kv = Object.fromEntries(s.props.split(',').filter(Boolean).map((x) => x.split('=')));
    let idx = 0;
    for (const p of layout) idx = idx * p.values.length + p.values.indexOf(kv[p.key]);
    block.bedrockIds[idx] = s.i;
  }
  blocks.push(block);
  nextState += count;
}
for (const p of props) if (p.type.kind === 'int' && p.values.some((v) => Number(v) > 255)) fail(`${p.marker}: int above u8`);
const stateCount = nextState;

// ---- neutral <-> Bedrock tables ----
// Bedrock -> neutral keeps the Java state closest to its block's default, dry first (anvil's rule,
// crates/acacia-anvil/README.md "Mapping"); flags say what the Bedrock state alone cannot tell.
const WATERLOGGED = 1, BLOCK_ENTITY = 2, OTHER = 4;
const distance = (id) => {
  const { block, combo } = javaCombo(id);
  const def = javaCombo(block.first + block.def).combo;
  return combo.reduce((d, v, i) => d + (v === def[i] ? 0 : block.layout[i].key === 'waterlogged' ? 100 : 1), 0);
};
const groups = new Map();
javaToBedrock.forEach((b, j) => (groups.get(b) ?? groups.set(b, []).get(b)).push(j));
const toNeutral = new Uint32Array(bedrock.length).fill(0xffffffff);
const flags = new Uint8Array(bedrock.length);
for (const [b, js] of groups) {
  const best = js.reduce((x, y) => (distance(y) < distance(x) ? y : x));
  toNeutral[b] = best;
  const blocksIn = new Set(js.map((j) => javaCombo(j).block));
  if (blocksIn.size > 1) flags[b] |= BLOCK_ENTITY;
  for (const j of js) {
    const a = javaCombo(j), c = javaCombo(best);
    if (a.block !== c.block) continue;
    a.combo.forEach((v, i) => { if (v !== c.combo[i]) flags[b] |= a.block.layout[i].key === 'waterlogged' ? WATERLOGGED : OTHER; });
  }
}
for (const blk of blocks.filter((x) => !x.java)) blk.bedrockIds.forEach((b, i) => { toNeutral[b] = blk.first + i; });
// Bedrock states Geyser never sends: the mapped state of the same block sharing the most properties.
const kvOf = (b) => new Set(bedrock[b].props.split(',').filter(Boolean));
bedrock.forEach((s, b) => {
  if (toNeutral[b] !== 0xffffffff) return;
  const mine = kvOf(b);
  let best = -1, score = -1;
  for (const o of groups.keys()) {
    if (bedrock[o].name !== s.name) continue;
    const sc = [...kvOf(o)].filter((x) => mine.has(x)).length;
    if (sc > score) { best = o; score = sc; }
  }
  if (best < 0) fail(`${s.name}: unmapped`);
  toNeutral[b] = toNeutral[best];
  flags[b] = flags[best] | OTHER;
});
const fromNeutral = new Uint32Array(stateCount);
javaToBedrock.forEach((b, j) => { fromNeutral[j] = b; });
for (const blk of blocks.filter((x) => !x.java)) blk.bedrockIds.forEach((b, i) => { fromNeutral[blk.first + i] = b; });

// Bedrock spellings that name exactly one neutral block, for `"noteblock".parse::<Block>()`.
const aliasTargets = new Map();
javaToBedrock.forEach((b, j) => {
  const n = bedrock[b].name;
  (aliasTargets.get(n) ?? aliasTargets.set(n, new Set()).get(n)).add(javaCombo(j).block);
});
const aliases = [...aliasTargets].filter(([n, s]) => s.size === 1 && !javaNames.has(n)).map(([n, s]) => [n, [...s][0]]);
for (const b of blocks) if (b.bedrockName && b.bedrockName !== b.name) aliases.push([b.bedrockName, b]);
aliases.sort(([a], [b]) => (a < b ? -1 : 1));

// ---- Rust output ----
const HEADER = '// @generated by crates/acacia-registry/tools/gen.mjs. Do not edit.\n';
const variants = blocks.map((b) => camel(b.name));
if (new Set(variants).size !== variants.length) fail('block variant names collide');
const valueType = (p) => (p.type.kind === 'bool' ? 'bool' : p.type.kind === 'int' ? 'u8' : p.type.kind === 'str' ? '&\'static str' : `value::${p.type.name}`);
const valueLit = (p, v) => (p.type.kind === 'bool' ? (v === 'true' || v === '1' ? 'true' : 'false')
  : p.type.kind === 'int' ? v : p.type.kind === 'str' ? JSON.stringify(v) : `value::${p.type.name}::${camel(v)}`);
const str = (s) => JSON.stringify(s);

let out = `${HEADER}use crate::block::BlockInfo;\nuse crate::prop::PropId;\n\n`;
out += '/// Every block kind: Java 26.3 blocks in registry order, then Bedrock-only blocks by name.\n';
out += '#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]\n#[repr(u16)]\npub enum Block {\n';
out += variants.map((v) => `    ${v},\n`).join('');
out += '}\n\n';
out += `pub(crate) const BLOCK_COUNT: usize = ${blocks.length};\npub(crate) const JAVA_BLOCK_COUNT: usize = ${blocks.filter((b) => b.java).length};\n`;
out += `pub(crate) const STATE_COUNT: u32 = ${stateCount};\npub(crate) const JAVA_STATE_COUNT: u32 = ${javaStateCount};\n\n`;
out += 'pub(crate) static BLOCKS: [BlockInfo; BLOCK_COUNT] = [\n';
out += blocks.map((b, i) => `    BlockInfo { block: Block::${variants[i]}, name: ${str(b.name)}, first: ${b.first}, default: ${b.first + b.def}, props: &[${b.layout.map((p) => `PropId::${p.marker}`).join(', ')}] },\n`).join('');
out += '];\n\n';
out += '/// Bedrock spellings naming exactly one block, sorted.\n';
out += `pub(crate) static BEDROCK_ALIASES: [(&str, Block); ${aliases.length}] = [\n`;
out += aliases.map(([n, b]) => `    (${str(n)}, Block::${camel(b.name)}),\n`).join('');
out += '];\n';
fs.mkdirSync(GEN, { recursive: true });
fs.writeFileSync(path.join(GEN, 'blocks.rs'), out);

out = HEADER;
for (const [name, vals] of valueTypes) {
  out += `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\npub enum ${name} {\n${vals.map((v) => `    ${camel(v)},\n`).join('')}}\n\n`;
  out += `impl ${name} {\n    pub const fn name(self) -> &'static str {\n        match self {\n`;
  out += vals.map((v) => `            Self::${camel(v)} => ${str(v)},\n`).join('');
  out += '        }\n    }\n}\n\n';
}
fs.writeFileSync(path.join(GEN, 'values.rs'), out);

out = `${HEADER}use crate::prop::{PropInfo, Property};\nuse crate::value;\n\n`;
out += '/// Every property, Java constants first, then those of Bedrock-only blocks.\n';
out += '#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n#[repr(u8)]\npub enum PropId {\n';
out += props.map((p) => `    ${p.marker},\n`).join('');
out += '}\n\n';
out += `pub(crate) static PROPS: [PropInfo; ${props.length}] = [\n`;
out += props.map((p) => `    PropInfo { key: ${str(p.key)}, values: &[${p.values.map(str).join(', ')}] },\n`).join('');
out += '];\n\n';
for (const p of props) {
  out += `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\npub struct ${p.marker};\n`;
  out += `impl Property for ${p.marker} {\n    type Value = ${valueType(p)};\n    const ID: PropId = PropId::${p.marker};\n`;
  out += `    const VALUES: &'static [${valueType(p)}] = &[${p.values.map((v) => valueLit(p, v)).join(', ')}];\n}\n`;
}
fs.writeFileSync(path.join(GEN, 'props.rs'), out);

// neutral.bin, parsed by acacia-world src/neutral.rs. Little endian:
//   magic "ANB1"; u32 n, n×(u32 neutral state, u8 flags) by Bedrock runtime id; u32 m, m×u32 Bedrock runtime id by neutral state
const buf = Buffer.alloc(4 + 4 + bedrock.length * 5 + 4 + stateCount * 4);
let o = buf.write('ANB1', 0);
o = buf.writeUInt32LE(bedrock.length, o);
for (let b = 0; b < bedrock.length; b++) { o = buf.writeUInt32LE(toNeutral[b], o); o = buf.writeUInt8(flags[b], o); }
o = buf.writeUInt32LE(stateCount, o);
for (let n = 0; n < stateCount; n++) o = buf.writeUInt32LE(fromNeutral[n], o);
fs.writeFileSync(NEUTRAL_BIN, buf);

const count = (f) => [...flags].filter((x) => x & f).length;
console.log(`${blocks.length} blocks (${extraNames.length} Bedrock-only), ${stateCount} states, ${props.length} properties, ${aliases.length} aliases`);
console.log(`Bedrock states needing context: waterlogged ${count(WATERLOGGED)}, block entity ${count(BLOCK_ENTITY)}, other ${count(OTHER)}`);
