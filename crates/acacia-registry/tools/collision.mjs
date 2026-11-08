// Writes src/generated/collision.bin from the block dump: node tools/collision.mjs tools/.cache
// Little endian: u32 states, states×u16 shape; u32 shapes, (shapes+1)×u32 first box; boxes×6×f64.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const { collision } = JSON.parse(fs.readFileSync(path.join(process.argv[2], 'java-26.3-block-dump.json'), 'utf8'));

const index = new Map();
const shapes = [];
const stateShape = collision.map((boxes) => {
  const key = JSON.stringify(boxes);
  if (!index.has(key)) {
    index.set(key, shapes.length);
    shapes.push(boxes);
  }
  return index.get(key);
});
if (shapes.length > 0xffff) throw new Error('shape index no longer fits u16');

const boxCount = shapes.reduce((n, s) => n + s.length, 0);
const buf = Buffer.alloc(4 + stateShape.length * 2 + 4 + (shapes.length + 1) * 4 + boxCount * 48);
let o = buf.writeUInt32LE(stateShape.length, 0);
for (const s of stateShape) o = buf.writeUInt16LE(s, o);
o = buf.writeUInt32LE(shapes.length, o);
let first = 0;
for (const s of shapes) {
  o = buf.writeUInt32LE(first, o);
  first += s.length;
}
o = buf.writeUInt32LE(first, o);
for (const s of shapes) for (const box of s) for (const v of box) o = buf.writeDoubleLE(v, o);
fs.writeFileSync(path.join(HERE, '../src/generated/collision.bin'), buf);
console.log(`${stateShape.length} states, ${shapes.length} shapes, ${boxCount} boxes, ${buf.length} bytes`);
