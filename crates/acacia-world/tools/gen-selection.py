"""Writes data/selection.bin: each vanilla state's selection (aim) boxes, in blocks.bin's runtime-id order.

usage: py -3 tools/gen-selection.py <shapes.json>
shapes.json is the static BDS read in the workspace's research/bds-selection-shapes (method: README there and
docs/selection-shapes.md). Layout, little-endian:
  b"BSL2", u32 shape count, per shape: u8 box count, 6 f32 per box (min xyz, max xyz),
  u32 state count, per state: u16 shape index, u8 random offset (0 none, 1 xz, 2 xyz; `_meta.random_offset`).
"""
import json
import os
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
BLOCKS = os.path.join(HERE, "..", "data", "blocks.bin")
OUT = os.path.join(HERE, "..", "data", "selection.bin")


def state_keys(b):
    """name|props of every state in blocks.bin (layout: gen-blocks.mjs)."""
    assert b[:4] == b"BWB3", "blocks.bin: bad magic"
    p = 4
    p += 1 + 4 * b[p]
    (n,) = struct.unpack_from("<I", b, p); p += 4
    strings = []
    for _ in range(n):
        (ln,) = struct.unpack_from("<H", b, p); p += 2
        strings.append(b[p:p + ln].decode()); p += ln
    (n,) = struct.unpack_from("<I", b, p); p += 4
    for _ in range(n):
        p += 1 + 24 * b[p]
    (n,) = struct.unpack_from("<H", b, p); p += 2 + 7 * n
    (n,) = struct.unpack_from("<I", b, p); p += 4
    keys = []
    for _ in range(n):
        name, props = struct.unpack_from("<HH", b, p); p += 17
        keys.append(f"{strings[name]}|{strings[props]}")
    return keys


def main():
    shapes = json.load(open(sys.argv[1]))
    keys = state_keys(open(BLOCKS, "rb").read())
    missing = [k for k in keys if k not in shapes]
    assert not missing, f"{len(missing)} states without a shape, e.g. {missing[:5]}"
    offsets = {f"minecraft:{name}": {"xz": 1, "xyz": 2}[kind] for name, kind in shapes["_meta"]["random_offset"].items()}
    table, index, ids = [], {}, []
    for k in keys:
        boxes = tuple(tuple(round(v, 6) for v in box) for box in shapes[k]["boxes"])
        if boxes not in index:
            index[boxes] = len(table)
            table.append(boxes)
        ids.append((index[boxes], offsets.get(k.split("|")[0], 0)))
    out = bytearray(b"BSL2") + struct.pack("<I", len(table))
    for boxes in table:
        out += struct.pack("<B", len(boxes))
        for box in boxes:
            out += struct.pack("<6f", *box)
    out += struct.pack("<I", len(ids)) + b"".join(struct.pack("<HB", i, o) for i, o in ids)
    open(OUT, "wb").write(out)
    print(f"{len(ids)} states, {len(table)} shapes, {len(out)} bytes -> {OUT}")


if __name__ == "__main__":
    main()
