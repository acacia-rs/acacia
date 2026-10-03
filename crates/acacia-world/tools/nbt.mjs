// Minimal NBT reader: big-endian (Java/Geyser resources) or little-endian (Bedrock disk format).
import { gunzipSync } from 'node:zlib';

export function readNbt(buf, { littleEndian = false } = {}) {
  if (buf[0] === 0x1f && buf[1] === 0x8b) buf = gunzipSync(buf);
  const dv = new DataView(buf.buffer, buf.byteOffset, buf.byteLength);
  const le = littleEndian;
  let o = 0;
  const u8 = () => buf[o++];
  const i16 = () => { const v = dv.getInt16(o, le); o += 2; return v; };
  const i32 = () => { const v = dv.getInt32(o, le); o += 4; return v; };
  const str = () => { const n = dv.getUint16(o, le); o += 2; const s = buf.toString('utf8', o, o + n); o += n; return s; };
  const payload = (t) => {
    switch (t) {
      case 1: return dv.getInt8(o++);
      case 2: return i16();
      case 3: return i32();
      case 4: { const v = dv.getBigInt64(o, le); o += 8; return v; }
      case 5: { const v = dv.getFloat32(o, le); o += 4; return v; }
      case 6: { const v = dv.getFloat64(o, le); o += 8; return v; }
      case 7: { const n = i32(); const v = buf.subarray(o, o + n); o += n; return Array.from(v); }
      case 8: return str();
      case 9: { const et = u8(); const n = i32(); const a = []; for (let i = 0; i < n; i++) a.push(payload(et)); return a; }
      case 10: {
        const m = {};
        for (;;) { const ct = u8(); if (ct === 0) return m; const k = str(); m[k] = payload(ct); }
      }
      case 11: { const n = i32(); const a = []; for (let i = 0; i < n; i++) a.push(i32()); return a; }
      case 12: { const n = i32(); const a = []; for (let i = 0; i < n; i++) { a.push(dv.getBigInt64(o, le)); o += 8; } return a; }
      default: throw new Error(`bad NBT tag ${t} at ${o}`);
    }
  };
  const rootType = u8();
  str();
  return payload(rootType);
}

// FNV-1 (multiply, then xor) 64-bit, as Bedrock/Geyser sort block names.
export function fnv1_64(s) {
  let h = 0xcbf29ce484222325n;
  for (const b of Buffer.from(s, 'utf8')) {
    h = (h * 0x100000001b3n) & 0xffffffffffffffffn;
    h ^= BigInt(b);
  }
  return h;
}
