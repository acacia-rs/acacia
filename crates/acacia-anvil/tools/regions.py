"""Anvil region files as {slot: (payload, timestamp)} (oracle.py's trim), and chunk payloads of compression
types 1 (gzip), 2 (zlib), 3 (none) and 4 (lz4-java's LZ4Block stream)."""
import gzip
import struct
import zlib


def _lz4_block(src, size):
    out, i = bytearray(), 0

    def length(n):
        nonlocal i
        if n == 15:
            while True:
                b = src[i]
                i += 1
                n += b
                if b != 255:
                    break
        return n

    while i < len(src):
        tok = src[i]
        i += 1
        lit = length(tok >> 4)
        out += src[i:i + lit]
        i += lit
        if i >= len(src):
            break
        off = src[i] | src[i + 1] << 8
        i += 2
        n, start = length(tok & 15) + 4, len(out) - off
        if off <= 0 or start < 0:
            raise ValueError("lz4 offset")
        if off >= n:
            out += out[start:start + n]
        else:
            for k in range(n):
                out.append(out[start + k])
    if len(out) != size:
        raise ValueError("lz4 block length")
    return bytes(out)


def lz4_blocks(data, decode):
    """Walks an LZ4Block stream to its end block; with `decode`, returns the data, else None."""
    out, i = bytearray(), 0
    while True:
        if data[i:i + 8] != b"LZ4Block" or len(data) < i + 21:
            raise ValueError("lz4 block header")
        method, (comp, orig, _) = data[i + 8] & 0xF0, struct.unpack_from("<iii", data, i + 9)
        if comp == 0 and orig == 0:
            return bytes(out) if decode else None
        body = data[i + 21:i + 21 + comp]
        if len(body) != comp or method not in (0x10, 0x20):
            raise ValueError("lz4 block")
        if decode:
            out += body if method == 0x10 else _lz4_block(body, orig)
        i += 21 + comp


def decode_payload(payload):
    """A slot's payload (compression byte + data) as chunk NBT."""
    kind, data = payload[0], payload[1:]
    if kind == 1:
        return gzip.decompress(data)
    if kind == 2:
        return zlib.decompress(data)
    if kind == 3:
        return data
    if kind == 4:
        return lz4_blocks(data, True)
    raise ValueError(f"chunk compression {kind}")


def payload_ok(payload):
    """Whether a payload is whole: zlib inflates (adler32 checked), an LZ4Block stream reaches its end block,
    uncompressed NBT starts as a compound."""
    try:
        if payload[0] == 4:
            lz4_blocks(payload[1:], False)
        elif payload[0] == 3:
            return payload[1:2] == b"\x0a"
        else:
            decode_payload(payload)
        return True
    except (ValueError, zlib.error, OSError, IndexError, struct.error):
        return False


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
