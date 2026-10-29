#!/usr/bin/env python3
"""Exports the structures a BDS world holds (`/structure save <name> ... disk` writes them into the world's
LevelDB as `structuretemplate_<namespace>:<name>`, not into files) as `<name>.mcstructure`.
Reads the write-ahead logs and the tables (Mojang's LevelDB: zlib blocks, compression id 2 or 4); the
latest sequence number wins, deletions included. Stdlib only.
usage: tools/ldb-structures.py <world db dir> <out dir> [name...]"""
import os
import struct
import sys
import zlib

PREFIX = b"structuretemplate_"


def varint(buf, i):
    v = shift = 0
    while True:
        b = buf[i]
        i += 1
        v |= (b & 0x7F) << shift
        if b < 0x80:
            return v, i
        shift += 7


def log_records(data):
    """Whole records of a LevelDB log file (32 KiB blocks, records split FIRST/MIDDLE/LAST)."""
    out, part, i = [], b"", 0
    while i + 7 <= len(data):
        if (i % 32768) + 7 > 32768:
            i += 32768 - i % 32768
            continue
        length, kind = struct.unpack_from("<HB", data, i + 4)
        chunk = data[i + 7:i + 7 + length]
        i += 7 + length
        if kind == 0 and length == 0:
            i += 32768 - i % 32768 if i % 32768 else 0
            continue
        if kind == 1:
            out.append(chunk)
        elif kind == 2:
            part = chunk
        elif kind == 3:
            part += chunk
        elif kind == 4:
            out.append(part + chunk)
    return out


def batch_entries(rec):
    seq, count = struct.unpack_from("<QI", rec, 0)
    i = 12
    for n in range(count):
        tag = rec[i]
        i += 1
        klen, i = varint(rec, i)
        key = rec[i:i + klen]
        i += klen
        val = None
        if tag == 1:
            vlen, i = varint(rec, i)
            val = rec[i:i + vlen]
            i += vlen
        yield key, seq + n, val


def block(data, offset, size):
    raw, kind = data[offset:offset + size], data[offset + size]
    if kind == 0:
        return raw
    if kind == 2:
        return zlib.decompress(raw)
    if kind == 4:
        return zlib.decompress(raw, -15)
    raise ValueError(f"block compression {kind}")


def block_entries(b):
    restarts = struct.unpack_from("<I", b, len(b) - 4)[0]
    end, i, key = len(b) - 4 - 4 * restarts, 0, b""
    while i < end:
        shared, i = varint(b, i)
        unshared, i = varint(b, i)
        vlen, i = varint(b, i)
        key = key[:shared] + b[i:i + unshared]
        i += unshared
        yield key, b[i:i + vlen]
        i += vlen


def table_entries(data):
    footer = data[-48:]
    _, j = varint(footer, 0)
    _, j = varint(footer, j)
    off, j = varint(footer, j)
    size, _ = varint(footer, j)
    for _, handle in block_entries(block(data, off, size)):
        boff, j = varint(handle, 0)
        bsize, _ = varint(handle, j)
        for ikey, val in block_entries(block(data, boff, bsize)):
            tag = struct.unpack_from("<Q", ikey, len(ikey) - 8)[0]
            yield ikey[:-8], tag >> 8, val if tag & 0xFF == 1 else None


def structures(db):
    latest = {}
    for f in sorted(os.listdir(db)):
        path = os.path.join(db, f)
        with open(path, "rb") as fh:
            data = fh.read()
        if f.endswith(".log"):
            entries = (e for r in log_records(data) for e in batch_entries(r))
        elif f.endswith(".ldb") or f.endswith(".sst"):
            entries = table_entries(data)
        else:
            continue
        for key, seq, val in entries:
            if key.startswith(PREFIX) and seq >= latest.get(key, (-1, None))[0]:
                latest[key] = (seq, val)
    return {k[len(PREFIX):].decode(): v for k, (_, v) in latest.items() if v is not None}


def main():
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    db, out, wanted = sys.argv[1], sys.argv[2], set(sys.argv[3:])
    os.makedirs(out, exist_ok=True)
    for full, val in sorted(structures(db).items()):
        name = full.split(":", 1)[-1]
        if wanted and name not in wanted:
            continue
        with open(os.path.join(out, name + ".mcstructure"), "wb") as fh:
            fh.write(val)
        print(f"{name}.mcstructure {len(val)} bytes")


if __name__ == "__main__":
    main()
