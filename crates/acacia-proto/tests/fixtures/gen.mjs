// Generates byte fixtures with the JS reference implementation (PrismarineJS bedrock-protocol).
//
// Values are drawn from protocol.json with a seeded PRNG, encoded by bedrock-protocol's serializer,
// and kept only if bedrock-protocol decodes and re-encodes them to the same bytes.
//
// Usage: npm i bedrock-protocol   (in any directory D)
//        BP_DIR=D node crates/acacia-proto/tests/fixtures/gen.mjs
// Output: packets/<name>.hex (one full packet per line, header included) and packets/<name>.json (decoded view).

import { createRequire } from 'node:module'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const here = path.dirname(fileURLToPath(import.meta.url))
const require = createRequire(path.join(process.env.BP_DIR ?? process.cwd(), 'noop.js'))
const { createSerializer, createDeserializer } = require('bedrock-protocol/src/transforms/serializer')

const VERSION = '1.26.51'
const SHIELD_ID = 387 // keep in sync with manual::DEFAULT_SHIELD_ITEM_ID
const SAMPLES = 4
const ATTEMPTS = 40
const protocol = JSON.parse(fs.readFileSync(path.join(here, '../../../../tools/codegen/data/protocol.json'), 'utf8'))
const types = protocol.types

// mulberry32
function rng (seed) {
  return () => {
    seed |= 0; seed = seed + 0x6D2B79F5 | 0
    let t = Math.imul(seed ^ seed >>> 15, 1 | seed)
    t = t + Math.imul(t ^ t >>> 7, 61 | t) ^ t
    return ((t ^ t >>> 14) >>> 0) / 4294967296
  }
}

let R = rng(1)
const int = (lo, hi) => lo + Math.floor(R() * (hi - lo + 1))
const pick = (xs) => xs[int(0, xs.length - 1)]
const big = (lo, hi) => BigInt(int(lo, hi))
const FLOATS = [0, 1.5, -2.25, 100.125, 0.5]
const str = () => Array.from({ length: int(0, 8) }, () => pick('abcxyz_ :.019'.split(''))).join('')
const buf = () => Buffer.from(Array.from({ length: int(0, 6) }, () => int(0, 255)))
const uuid = () => [8, 4, 4, 4, 12].map(n => Array.from({ length: n }, () => int(0, 15).toString(16)).join('')).join('-')

// No intArray/longArray: prismarine-nbt writes their elements as fixed-width ints in the varint flavour,
// whereas Mojang (and acacia-proto) use zigzag varints. See docs/proto.md.
function nbtValue (depth) {
  const t = pick(depth > 1 ? ['byte', 'short', 'int', 'long', 'string', 'double'] : ['int', 'string', 'list', 'compound', 'float'])
  switch (t) {
    case 'byte': return { type: t, value: int(-100, 100) }
    case 'short': return { type: t, value: int(-1000, 1000) }
    case 'int': return { type: t, value: int(-100000, 100000) }
    case 'long': return { type: t, value: [0, int(0, 1000)] }
    case 'float': case 'double': return { type: t, value: pick(FLOATS) }
    case 'string': return { type: t, value: str() }
    case 'list': return { type: t, value: { type: 'int', value: [int(0, 50), int(0, 50)] } }
    case 'compound': return { type: t, value: compound(depth + 1) }
  }
}

function compound (depth) {
  const out = {}
  for (let i = 0; i < int(0, 3); i++) out['k' + String.fromCharCode(97 + i)] = nbtValue(depth)
  return out
}

const nbtRoot = () => ({ type: 'compound', name: '', value: compound(0) })

const NATIVE = {
  varint: () => int(0, 20000),
  varint64: () => big(0, 1e9),
  varint128: () => big(0, 1e9),
  zigzag32: () => int(-20000, 20000),
  zigzag64: () => big(-1e9, 1e9),
  u8: () => int(0, 255),
  i8: () => int(-128, 127),
  u16: () => int(0, 65535), lu16: () => int(0, 65535),
  i16: () => int(-32768, 32767), li16: () => int(-32768, 32767),
  u32: () => int(0, 2 ** 31), lu32: () => int(0, 2 ** 31),
  i32: () => int(-(2 ** 31), 2 ** 31 - 1), li32: () => int(-(2 ** 31), 2 ** 31 - 1),
  u64: () => big(0, 1e12), lu64: () => big(0, 1e12),
  i64: () => big(-1e12, 1e12), li64: () => big(-1e12, 1e12),
  f32: () => pick(FLOATS), lf32: () => pick(FLOATS), f64: () => pick(FLOATS), lf64: () => pick(FLOATS),
  bool: () => R() < 0.5,
  uuid,
  byterot: () => int(0, 255) * (360 / 256),
  restBuffer: buf,
  nbt: nbtRoot,
  lnbt: nbtRoot,
  nbtLoop: () => Array.from({ length: int(0, 2) }, nbtRoot),
  void: () => undefined
}

// Resolves a compareTo/count path against the stack of containers being built (innermost last).
function lookup (scopes, p) {
  let up = 0
  while (p.startsWith('../')) { up++; p = p.slice(3) }
  for (let i = scopes.length - 1 - up; i >= 0; i--) if (p in scopes[i]) return scopes[i][p]
  throw new Error('unresolved ' + p)
}

function gen (type, scopes, depth) {
  if (typeof type === 'string') {
    if (NATIVE[type]) return NATIVE[type]()
    if (!types[type]) throw new Error('unknown type ' + type)
    return gen(types[type], scopes, depth)
  }
  const [kind, o] = type
  const len = () => depth > 4 ? 0 : int(0, depth > 2 ? 1 : 3)
  switch (kind) {
    case 'container': {
      const obj = {}
      const counts = new Set(o.map(f => Array.isArray(f.type) && f.type[0] === 'array' && typeof f.type[1].count === 'string' ? f.type[1].count : null))
      scopes.push(obj)
      for (const f of o) {
        const v = counts.has(f.name) ? len() : gen(f.type, scopes, depth + 1)
        if (f.anon) Object.assign(obj, v)
        else obj[f.name] = v
      }
      scopes.pop()
      return obj
    }
    case 'array': case 'maybeIncompleteArray': {
      const n = typeof o.count === 'number' ? o.count : typeof o.count === 'string' ? lookup(scopes, o.count) : len()
      return Array.from({ length: n }, () => gen(o.type, scopes, depth + 1))
    }
    case 'option': return depth > 5 || R() < 0.4 ? undefined : gen(o, scopes, depth + 1)
    case 'switch': {
      const v = lookup(scopes, o.compareTo)
      const key = Object.keys(o.fields).find(k => k === '/ShieldItemID' ? v === SHIELD_ID : k === String(v))
      return gen(key !== undefined ? o.fields[key] : (o.default ?? 'void'), scopes, depth)
    }
    case 'mapper': return pick(Object.values(o.mappings))
    case 'bitflags': return { _value: o.big ? big(0, 1e9) : int(0, 255) }
    case 'bitfield': return Object.fromEntries(o.map(f => [f.name, int(0, 2 ** f.size - 1)]))
    case 'encapsulated': case 'optionalOnRemaining': return gen(o.type, scopes, depth)
    case 'pstring': return str()
    case 'buffer': return buf()
    default: throw new Error('unsupported kind ' + kind)
  }
}

const jsonable = (k, v) => typeof v === 'bigint' ? v.toString() : (v && v.type === 'Buffer' ? Buffer.from(v.data).toString('hex') : v)

const ser = createSerializer(VERSION)
const des = createDeserializer(VERSION)
for (const p of [ser.proto, des.proto]) p.setVariable('ShieldItemID', SHIELD_ID)

const mcpe = types.mcpe_packet[1]
const names = Object.values(mcpe[0].type[1].mappings)
const outDir = path.join(here, 'packets')
fs.rmSync(outDir, { recursive: true, force: true })
fs.mkdirSync(outDir, { recursive: true })

const failed = []
for (const name of names) {
  const hex = []
  const decoded = []
  for (let seed = 0; seed < ATTEMPTS && hex.length < SAMPLES; seed++) {
    R = rng(seed * 7919 + name.length)
    try {
      const params = gen(mcpe[1].type[1].fields[name], [], 0)
      const bytes = ser.createPacketBuffer({ name, params })
      const back = des.parsePacketBuffer(bytes)
      if (back.data.name !== name || !ser.createPacketBuffer(back.data).equals(bytes)) continue
      if (hex.includes(bytes.toString('hex'))) continue
      hex.push(bytes.toString('hex'))
      decoded.push(back.data.params)
    } catch (e) {
      if (seed === ATTEMPTS - 1 && !hex.length) failed.push(`${name}: ${e.message.split('\n')[0]}`)
    }
  }
  if (!hex.length) { if (!failed.find(f => f.startsWith(name + ':'))) failed.push(`${name}: no JS-stable sample`); continue }
  fs.writeFileSync(path.join(outDir, name + '.hex'), hex.join('\n') + '\n')
  fs.writeFileSync(path.join(outDir, name + '.json'), JSON.stringify(decoded, jsonable, 1) + '\n')
}
console.log(`wrote fixtures for ${names.length - failed.length}/${names.length} packets`)
if (failed.length) console.log('no fixture:\n  ' + failed.join('\n  '))
