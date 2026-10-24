// Generates data/mapping.bin: the Java block-state table and the Bedrock → Java state and biome maps.
// Usage: node crates/acacia-anvil/tools/gen-mapping.mjs [cacheDir]   (README.md "Data sources")
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { readNbt } from './nbt.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const CACHE = process.argv[2] ?? path.join(HERE, '.cache');
const OUT = path.join(HERE, '..', 'data', 'mapping.bin');
// GeyserMC/mappings for Java 26.3 + Bedrock 26.50 (MIT).
const GEYSER = 'https://raw.githubusercontent.com/GeyserMC/mappings/81700ab543c1f4e9f9041f58dac4154ecb0d0c8b';
// Vanilla 26.3 data report: java -DbundlerMainClass=net.minecraft.data.Main -jar server-26.3.jar --reports
const JAVA_REPORT = 'java-26.3-blocks.json';
// Java biomes sharing one Bedrock id: the one a Bedrock id most likely came from.
const BIOME_PREFERRED = new Set(['minecraft:river', 'minecraft:the_end']);

async function load(name, url) {
  const file = path.join(CACHE, name);
  if (!fs.existsSync(file)) {
    if (!url) throw new Error(`${file} missing: see README.md "Data sources"`);
    const res = await fetch(url);
    if (!res.ok) throw new Error(`${url}: ${res.status}`);
    fs.mkdirSync(CACHE, { recursive: true });
    fs.writeFileSync(file, Buffer.from(await res.arrayBuffer()));
  }
  return fs.readFileSync(file);
}

const report = JSON.parse(await load(JAVA_REPORT));
const geyser = readNbt(await load('blocks.nbt', `${GEYSER}/blocks.nbt`)).bedrock_mappings;
const biomes = JSON.parse(await load('biomes.json', `${GEYSER}/biomes.json`));

// The report lists properties by name, not in the order states are enumerated: order them by stride.
const stride = (states, k) => { let n = 1; while (n < states.length && states[n].properties[k] === states[0].properties[k]) n++; return n; };
// Java blocks in state-id order. A block's states are the product of its properties, last one fastest.
const blocks = Object.entries(report)
  .map(([name, b]) => {
    const props = Object.entries(b.properties ?? {}).sort(([x], [y]) => stride(b.states, y) - stride(b.states, x));
    return { name, props, states: b.states };
  })
  .sort((a, b) => a.states[0].id - b.states[0].id);
const javaStates = [];
for (const b of blocks) {
  const base = b.states[0].id;
  if (base !== javaStates.length) throw new Error(`${b.name}: ids not contiguous`);
  b.states.forEach((s, i) => {
    let rest = i;
    const combo = [];
    for (let p = b.props.length - 1; p >= 0; p--) {
      const [k, vals] = b.props[p];
      combo.unshift([k, vals[rest % vals.length]]);
      rest = Math.floor(rest / vals.length);
    }
    for (const [k, v] of combo) if (s.properties[k] !== v) throw new Error(`${b.name} state ${i}: order`);
    if (s.default) b.defaultIndex = i;
    javaStates.push({ block: b, index: i, combo });
  });
}
if (javaStates.length !== geyser.length) throw new Error(`${javaStates.length} Java states, ${geyser.length} in Geyser`);

// Bedrock state key in acacia-world's form: name, then `k=v` sorted by key, booleans as 0/1.
const bedrockKey = (entry, java) => {
  const name = `minecraft:${entry.bedrock_identifier ?? java.block.name.slice('minecraft:'.length)}`;
  const st = entry.state ?? {};
  return `${name}|${Object.keys(st).sort().map((k) => `${k}=${st[k]}`).join(',')}`;
};
// Many Java states share a Bedrock state; keep the one closest to its block's defaults, waterlogged=false
// first (the converter sets waterlogging from Bedrock's liquid layer).
const distance = (j) => {
  const def = javaStates[j.block.states[0].id + j.block.defaultIndex].combo;
  let d = 0;
  j.combo.forEach(([k, v], i) => { if (v !== def[i][1]) d += k === 'waterlogged' ? 100 : 1; });
  return d;
};
const b2j = new Map();
geyser.forEach((entry, id) => {
  const key = bedrockKey(entry, javaStates[id]);
  const prev = b2j.get(key);
  if (prev === undefined || distance(javaStates[id]) < distance(javaStates[prev])) b2j.set(key, id);
});

// Java properties Bedrock does not carry: per block, those that differ among Java states sharing a
// Bedrock state. Written to data/ambiguous.txt; the converter rebuilds the visible ones (README.md).
const groups = new Map();
geyser.forEach((entry, id) => { const key = bedrockKey(entry, javaStates[id]); (groups.get(key) ?? groups.set(key, []).get(key)).push(id); });
const lost = new Map();
for (const ids of groups.values()) {
  if (ids.length < 2) continue;
  for (const id of ids) {
    const j = javaStates[id];
    const first = javaStates[ids[0]];
    const set = lost.get(j.block.name) ?? lost.set(j.block.name, new Set()).get(j.block.name);
    j.combo.forEach(([k, v], i) => { if (first.block === j.block && first.combo[i][1] !== v) set.add(k); });
    if (first.block !== j.block) set.add('<block>');
  }
}
const lostLines = [...lost].filter(([, s]) => s.size).map(([n, s]) => `${n} ${[...s].sort().join(',')}`).sort();
fs.writeFileSync(path.join(HERE, '..', 'data', 'ambiguous.txt'), `${lostLines.join('\n')}\n`);

const biomeById = new Map();
for (const [name, { bedrock_id: id }] of Object.entries(biomes).sort()) {
  if (!biomeById.has(id) || BIOME_PREFERRED.has(name)) biomeById.set(id, name);
}

// Layout (little endian), parsed by src/mapping/blob.rs:
//   magic "AJM1"; strings: u32 n, n×(u16 len, utf8)
//   java blocks: u32 n, n×(u32 name, u32 default index, u8 props, props×(u32 key, u8 values, values×u32))
//   bedrock → java: u32 n, n×(u32 name, u32 props, u32 java state id)
//   java → bedrock (Geyser's forward map): u32 java states, each u16 index into the list above
//   biomes: u32 n, n×(u32 bedrock id, u32 java name)
const strings = new Map();
const str = (s) => { if (!strings.has(s)) strings.set(s, strings.size); return strings.get(s); };
const body = [];
const u8 = (v) => body.push(Buffer.from([v]));
const u32 = (v) => { const b = Buffer.alloc(4); b.writeUInt32LE(v); body.push(b); };
u32(blocks.length);
for (const b of blocks) {
  u32(str(b.name)); u32(b.defaultIndex); u8(b.props.length);
  for (const [k, vals] of b.props) { u32(str(k)); u8(vals.length); vals.forEach((v) => u32(str(v))); }
}
const bedrockList = [...b2j].sort((a, b) => a[1] - b[1]);
u32(bedrockList.length);
for (const [key, id] of bedrockList) {
  const [name, props] = key.split('|');
  u32(str(name)); u32(str(props)); u32(id);
}
const bedrockIndex = new Map(bedrockList.map(([key], i) => [key, i]));
u32(geyser.length);
geyser.forEach((entry, id) => { const b = Buffer.alloc(2); b.writeUInt16LE(bedrockIndex.get(bedrockKey(entry, javaStates[id]))); body.push(b); });
u32(biomeById.size);
for (const [id, name] of [...biomeById].sort((a, b) => a[0] - b[0])) { u32(id); u32(str(name)); }

const head = [Buffer.from('AJM1'), Buffer.alloc(4)];
head[1].writeUInt32LE(strings.size);
for (const s of strings.keys()) { const b = Buffer.from(s, 'utf8'); const l = Buffer.alloc(2); l.writeUInt16LE(b.length); head.push(l, b); }
fs.mkdirSync(path.dirname(OUT), { recursive: true });
fs.writeFileSync(OUT, Buffer.concat([...head, ...body]));
console.log(`${blocks.length} Java blocks, ${javaStates.length} states; ${b2j.size} Bedrock states; ${biomeById.size} biomes; ${fs.statSync(OUT).size} bytes`);
