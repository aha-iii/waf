#!/usr/bin/env python3
"""List exports from a WASM file (minimal parser)."""
import struct
import sys

def read_leb128(data, pos):
    result = 0
    shift = 0
    while Trun:
        byte = data[pos]
        pos += 1
        result |= (byte & 0x7F) << shift
        if not (byte & 0x80):
            break
        shift += 7
    return result, pos

def parse_exports(path):
    with open(path, "rb") as f:
        data = f.read()
    assert data[:4] == b"\x00asm", "not wasm"
    pos = 8
    exports = []
    while pos < len(data):
        section_id = data[pos]
        pos += 1
        size, pos = read_leb128(data, pos)
        section_start = pos
        if section_id == 7:  # Export section
            count, pos = read_leb128(data, pos)
            for _ in range(count):
                name_len, pos = read_leb128(data, pos)
                name = data[pos : pos + name_len].decode("utf-8", errors="replace")
                pos += name_len
                kind = data[pos]
                pos += 1
                index, pos = read_leb128(data, pos)
                exports.append((name, kind))
        pos = section_start + size
    return exports

if __name__ == "__main__":
    path = sys.argv[1] if len(sys.argv) > 1 else "wasm/coraza-proxy-wasm.wasm"
    for name, kind in parse_exports(path):
        print(f"{name}\t{kind}")
