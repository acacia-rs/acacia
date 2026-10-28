"""Prints the block state, sky and block light at positions in one or more Anvil worlds (overworld), to compare
a diff-render cell between the server's world and the converted one.

Usage: py -3 crates/acacia-anvil/tools/mca_block.py <world> [<world> ...] -- x,y,z [x,y,z ...]
(ANVIL_DIM=the_nether|the_end for another dimension)
"""
import os
import struct
import sys
from pathlib import Path

from regions import decode_payload


def nbt(buf: bytes):
    p = 0

    def take(n):
        nonlocal p
        p += n
        return buf[p - n:p]

    def string():
        return take(struct.unpack(">H", take(2))[0]).decode("utf-8", "replace")

    def payload(t):
        if t == 1: return struct.unpack(">b", take(1))[0]
        if t == 2: return struct.unpack(">h", take(2))[0]
        if t == 3: return struct.unpack(">i", take(4))[0]
        if t == 4: return struct.unpack(">q", take(8))[0]
        if t == 5: return struct.unpack(">f", take(4))[0]
        if t == 6: return struct.unpack(">d", take(8))[0]
        if t == 7: return take(struct.unpack(">i", take(4))[0])
        if t == 8: return string()
        if t == 9:
            et, n = take(1)[0], struct.unpack(">i", take(4))[0]
            return [payload(et) for _ in range(n)]
        if t == 10:
            out = {}
            while (tt := take(1)[0]) != 0:
                name = string()
                out[name] = payload(tt)
            return out
        if t == 11:
            n = struct.unpack(">i", take(4))[0]
            return list(struct.unpack(f">{n}i", take(4 * n)))
        if t == 12:
            n = struct.unpack(">i", take(4))[0]
            return list(struct.unpack(f">{n}q", take(8 * n)))
        raise ValueError(t)

    t = take(1)[0]
    string()
    return payload(t)


def chunk(world: Path, cx: int, cz: int):
    for d in (f"dimensions/minecraft/{os.environ.get('ANVIL_DIM', 'overworld')}/region", "region"):
        f = world / d / f"r.{cx >> 5}.{cz >> 5}.mca"
        if f.exists():
            data = f.read_bytes()
            i = (cx & 31) + (cz & 31) * 32
            loc = struct.unpack_from(">I", data, i * 4)[0]
            if not loc & 0xFF:
                return None
            off = (loc >> 8) * 4096
            length = struct.unpack_from(">I", data, off)[0]
            return nbt(decode_payload(data[off + 4: off + 4 + length]))
    return None


def entry(e):
    if isinstance(e, str):
        return e + "(default)"
    name = e.get("Name") or e.get("id")
    props = e.get("Properties") or e.get("properties") or {}
    return name + ("[" + ",".join(f"{k}={v}" for k, v in sorted(props.items())) + "]" if props else "")


def block(world: Path, x: int, y: int, z: int) -> str:
    c = chunk(world, x >> 4, z >> 4)
    if c is None:
        return "no chunk"
    sec = next((s for s in c["sections"] if s["Y"] == y >> 4), None)
    if sec is None:
        return "no section"
    i = ((y & 15) << 8) | ((z & 15) << 4) | (x & 15)
    bs = sec.get("block_states", {})
    pal, data = bs.get("palette", []), bs.get("data")
    if len(pal) > 1 and data:
        bits = max(4, (len(pal) - 1).bit_length())
        per = 64 // bits
        idx = (data[i // per] & (1 << 64) - 1) >> ((i % per) * bits) & ((1 << bits) - 1)
    else:
        idx = 0
    nib = lambda arr: (arr[i >> 1] >> ((i & 1) * 4)) & 15 if arr and len(arr) == 2048 else "-"
    return f"{entry(pal[idx]) if pal else '?'} sky={nib(sec.get('SkyLight'))} block={nib(sec.get('BlockLight'))}"


def main():
    args = sys.argv[1:]
    worlds, cells = args[: args.index("--")], args[args.index("--") + 1:]
    for cell in cells:
        x, y, z = map(int, cell.split(","))
        for w in worlds:
            print(f"{cell:>14} {Path(w).parent.name:>10}: {block(Path(w), x, y, z)}")


if __name__ == "__main__":
    main()
