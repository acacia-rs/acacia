"""Anvil region files as {slot: (payload, timestamp)} (oracle.py's trim)."""
import struct


def read_region(path):
    data = path.read_bytes()
    chunks = {}
    for i in range(1024):
        loc = struct.unpack_from(">I", data, i * 4)[0]
        off, count = (loc >> 8) * 4096, loc & 0xFF
        if count and off + 4 <= len(data):
            length = struct.unpack_from(">I", data, off)[0]
            ts = struct.unpack_from(">I", data, 4096 + i * 4)[0]
            chunks[i] = (data[off + 4: off + 4 + length], ts)
    return chunks


def write_region(path, chunks):
    header, body, sector = bytearray(8192), bytearray(), 2
    for i, (payload, ts) in sorted(chunks.items()):
        blob = struct.pack(">I", len(payload)) + payload
        blob += b"\0" * (-len(blob) % 4096)
        struct.pack_into(">I", header, i * 4, (sector << 8) | (len(blob) // 4096))
        struct.pack_into(">I", header, 4096 + i * 4, ts)
        body += blob
        sector += len(blob) // 4096
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(bytes(header + body))
