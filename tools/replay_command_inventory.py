#!/usr/bin/env python3
"""Inventory command tags without assigning gameplay names.

The replay inspector proves framing.  This companion keeps the same parser but
counts the observed opcode/custom-type/custom-length combinations and receiver
kind combinations.  It deliberately emits no replay payloads, identifiers, or
binary data, so its output is suitable for comparing corpora from different
builds without publishing the corpus itself.
"""

from __future__ import annotations

import collections
import json
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from replay_inspect import ReplayError, chunk_at, decode_command, header  # noqa: E402


def inspect(path: Path) -> dict[str, object]:
    data = path.read_bytes()
    meta = header(data)
    foldpost = chunk_at(data, 120, len(data))
    scenario = chunk_at(data, foldpost.end + 36, len(data))
    foldinfo = chunk_at(data, scenario.end, len(data))
    pos = foldinfo.end
    counts: collections.Counter[tuple[int, int, int]] = collections.Counter()
    receiver_kinds: collections.Counter[tuple[int, int]] = collections.Counter()
    command_counts: collections.Counter[int] = collections.Counter()
    sync_records = commands = bundles = 0

    while pos < len(data):
        if pos + 8 > len(data):
            raise ReplayError(f"short outer record at 0x{pos:x}")
        record_type, payload_length = struct.unpack_from("<II", data, pos)
        payload_start = pos + 8
        payload_end = payload_start + payload_length
        if payload_end > len(data):
            raise ReplayError(f"outer record at 0x{pos:x} runs past EOF")
        payload = data[payload_start:payload_end]
        pos = payload_end
        if record_type != 0:
            continue
        if len(payload) < 17 or payload[0] != 0x20:
            raise ReplayError("sync payload has no 0x20 marker/header")
        _, _, _, bundle_count = struct.unpack_from("<IIII", payload, 1)
        sync_records += 1
        bundles += bundle_count
        cursor = 17
        for _ in range(bundle_count):
            if cursor + 12 > len(payload):
                raise ReplayError("short bundle header")
            declared = struct.unpack_from("<I", payload, cursor + 8)[0]
            cursor += 12
            if cursor >= len(payload):
                raise ReplayError("bundle has no length echo")
            echo = payload[cursor]
            cursor += 1
            if echo != declared & 0xFF:
                raise ReplayError("bundle length echo mismatch")
            bundle_end = cursor + declared
            if bundle_end > len(payload):
                raise ReplayError("bundle runs past sync payload")
            while cursor < bundle_end:
                if cursor + 2 > bundle_end:
                    raise ReplayError("short command length")
                length = struct.unpack_from("<H", payload, cursor)[0]
                if length < 2 or cursor + length > bundle_end:
                    raise ReplayError("command runs past bundle")
                command = decode_command(payload[cursor + 2 : cursor + length])
                opcode = int(command["opcode"])
                custom_type = int(command["custom_type"])
                custom_length = int(command["custom_length"])
                counts[(opcode, custom_type, custom_length)] += 1
                command_counts[opcode] += 1
                for receiver in command["receivers"]:
                    receiver_kinds[(opcode, (int(receiver) >> 28) & 0xF)] += 1
                commands += 1
                cursor += length
            if cursor != bundle_end:
                raise ReplayError("bundle has an unconsumed tail")
        if cursor != len(payload):
            raise ReplayError("sync payload has an unconsumed tail")

    return {
        "file": str(path),
        "version": int(meta["version"]),
        "mod_checksum": meta["mod_checksum"],
        "sync_records": sync_records,
        "bundles": bundles,
        "commands": commands,
        "opcodes": {str(k): v for k, v in sorted(command_counts.items())},
        "opcode_custom_lengths": {
            f"{opcode}:{custom_type}:{length}": count
            for (opcode, custom_type, length), count in sorted(counts.items())
        },
        "opcode_receiver_kinds": {
            f"{opcode}:{kind}": count
            for (opcode, kind), count in sorted(receiver_kinds.items())
        },
    }


def main(argv: list[str]) -> int:
    try:
        for name in argv:
            print(json.dumps(inspect(Path(name)), sort_keys=True))
    except (OSError, ReplayError, struct.error) as exc:
        print(f"{exc}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
