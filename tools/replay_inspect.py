#!/usr/bin/env python3
"""Inspect the stable framing of a Dawn of War II .rec file.

This intentionally does not try to name every action or attribute.  It checks
file boundaries, reports the chunk tree, and decodes the command framing that
has been verified against the public sample replays.  It is dependency free so
that it can also be used while investigating a replay from a different build.
"""

from __future__ import annotations

import argparse
import json
import struct
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Iterable


class ReplayError(ValueError):
    """A replay failed a structural check."""


@dataclass
class Chunk:
    name: str
    offset: int
    version: int
    size: int
    end: int
    children: list["Chunk"] = field(default_factory=list)


def u16(data: bytes, offset: int, *, endian: str = "<") -> int:
    if offset < 0 or offset + 2 > len(data):
        raise ReplayError(f"u16 outside file at 0x{offset:x}")
    return struct.unpack_from(endian + "H", data, offset)[0]


def u32(data: bytes, offset: int, *, endian: str = "<") -> int:
    if offset < 0 or offset + 4 > len(data):
        raise ReplayError(f"u32 outside file at 0x{offset:x}")
    return struct.unpack_from(endian + "I", data, offset)[0]


def chunk_at(data: bytes, offset: int, limit: int) -> Chunk:
    # Relic Chunky uses an eight-byte name, version, size, and twelve bytes of
    # per-chunk metadata.  The size is the payload size, after those 28 bytes.
    if offset + 28 > limit:
        raise ReplayError(f"short chunk header at 0x{offset:x}")
    raw_name = data[offset : offset + 8]
    name = raw_name.rstrip(b"\0").decode("ascii", errors="replace")
    version = u32(data, offset + 8)
    size = u32(data, offset + 12)
    end = offset + 28 + size
    if end > limit:
        raise ReplayError(
            f"chunk {name!r} at 0x{offset:x} ends at 0x{end:x}, "
            f"past limit 0x{limit:x}"
        )
    chunk = Chunk(name, offset, version, size, end)
    # These are containers in the observed files.  Other chunks contain data,
    # including DATAINFO and DATABASE, and must not be recursively guessed.
    if name.startswith("FOLD"):
        child = offset + 28
        while child < end:
            chunk.children.append(chunk_at(data, child, end))
            child = chunk.children[-1].end
        if child != end:
            raise ReplayError(f"container {name!r} has an incomplete tail")
    return chunk


def header(data: bytes) -> dict[str, object]:
    if len(data) < 84:
        raise ReplayError("file is shorter than the 84-byte replay header")
    magic = data[12:20]
    if magic != b"DOW2_REC":
        raise ReplayError(f"unexpected magic {magic!r}")
    raw_date = data[20:84].decode("utf-16le", errors="replace")
    return {
        "version": u32(data, 0),
        "mod_checksum": f"0x{u32(data, 4):08x}",
        "reserved": u32(data, 8),
        "magic": magic.decode("ascii"),
        "date": raw_date.split("\0", 1)[0],
    }


def flatten(chunks: Iterable[Chunk]) -> Iterable[Chunk]:
    for chunk in chunks:
        yield chunk
        yield from flatten(chunk.children)


CHUNKY_MAGIC = b"Relic Chunky\r\n\x1a\0"


def chunky_signature(data: bytes, offset: int) -> dict[str, int]:
    """Validate one 36-byte Relic Chunky v3 signature block.

    The replay writer drives two Chunky instances over the same file, so this
    block appears twice (at 84 and again between FOLDPOST and DATASDSC).  The
    field names come from Chunky::ConstructInternal in the analysed build.
    """
    if offset + 36 > len(data):
        raise ReplayError(f"short Chunky signature at 0x{offset:x}")
    if data[offset : offset + 16] != CHUNKY_MAGIC:
        raise ReplayError(f"missing Relic Chunky magic at 0x{offset:x}")
    version = u32(data, offset + 16)
    platform = u32(data, offset + 20)
    signature_size = u32(data, offset + 24)
    header_size = u32(data, offset + 28)
    if version != 3 or signature_size != 0x24 or header_size != 0x1C:
        raise ReplayError(
            f"unsupported Chunky signature at 0x{offset:x}: "
            f"version {version}, sizes 0x{signature_size:x}/0x{header_size:x}"
        )
    return {"version": version, "platform": platform}


def wide_string(data: bytes, pos: int, limit: int) -> tuple[str, int]:
    count = u32(data, pos)
    end = pos + 4 + 2 * count
    if end > limit:
        raise ReplayError(f"wide string at 0x{pos:x} runs past 0x{limit:x}")
    return data[pos + 4 : end].decode("utf-16le", errors="replace"), end


def narrow_string(data: bytes, pos: int, limit: int) -> tuple[str, int]:
    count = u32(data, pos)
    end = pos + 4 + count
    if end > limit:
        raise ReplayError(f"string at 0x{pos:x} runs past 0x{limit:x}")
    return data[pos + 4 : end].decode("latin-1"), end


def parse_player_info(data: bytes, chunk: Chunk) -> dict[str, object]:
    """Decode one DATAINFO player block (writer at DOW2 0x41a9c1).

    Field names follow docs/replay-format.md; numeric fields whose meaning is
    only a corpus observation keep neutral names.
    """
    if chunk.version != 7:
        raise ReplayError(f"unsupported DATAINFO version {chunk.version}")
    pos, limit = chunk.offset + 28, chunk.end
    name, pos = wide_string(data, pos, limit)
    kind = u32(data, pos)
    team = u32(data, pos + 4)
    pos += 8
    race, pos = narrow_string(data, pos, limit)
    if pos + 8 > limit:
        raise ReplayError("DATAINFO profile ID runs past chunk")
    profile_id = struct.unpack_from("<Q", data, pos)[0]
    pos += 8
    rank = u32(data, pos)
    observed7 = u32(data, pos + 4)
    observed8 = u32(data, pos + 8)
    observed9 = u32(data, pos + 12)
    pos += 16
    if pos + 1 > limit:
        raise ReplayError("DATAINFO flag byte runs past chunk")
    observed_flag = data[pos]
    pos += 1
    rank_repeat = u32(data, pos)
    pos += 4
    if pos + 5 > limit:
        raise ReplayError("DATAINFO appearance bytes run past chunk")
    appearance = list(data[pos : pos + 5])
    pos += 5
    badge, pos = narrow_string(data, pos, limit)
    badge_crc = u32(data, pos)
    pos += 4
    army_name, pos = wide_string(data, pos, limit)
    race_id = u32(data, pos)
    pos += 4
    if pos != limit:
        raise ReplayError(
            f"DATAINFO has {limit - pos} undecoded trailing bytes"
        )
    if rank_repeat != rank:
        raise ReplayError("DATAINFO duplicated rank fields disagree")
    return {
        "name": name,
        "kind": kind,
        "team": team,
        "race": race,
        "profile_id": profile_id,
        "rank": rank,
        "observed7": observed7,
        "observed8": observed8,
        "observed9": observed9,
        "observed_flag": observed_flag,
        "appearance": appearance,
        "badge": badge,
        "badge_crc": badge_crc,
        "army_name": army_name,
        "race_id": race_id,
    }


def parse_chat(payload: bytes, stats: dict[str, object]) -> None:
    """Decode one type-1 record (serialized RecordedChatCmd)."""
    if len(payload) < 8:
        raise ReplayError("chat payload is shorter than its inner framing")
    inner_kind, inner_size = struct.unpack_from("<II", payload, 0)
    if inner_kind != 1:
        raise ReplayError(f"chat inner kind {inner_kind} != 1")
    if inner_size != len(payload) - 8:
        raise ReplayError(
            f"chat inner size {inner_size} != payload size - 8"
        )
    pos = 8
    _name, pos = wide_string(payload, pos, len(payload))
    if pos + 12 > len(payload):
        raise ReplayError("chat dwords run past payload")
    sender_key, zero_field, team_flag = struct.unpack_from("<III", payload, pos)
    pos += 12
    _text, pos = wide_string(payload, pos, len(payload))
    if pos != len(payload):
        raise ReplayError("chat payload has an unconsumed tail")
    if not 1000 <= sender_key < 1000 + 128:
        raise ReplayError(f"chat sender key {sender_key} outside 1000+slot range")
    stats["chat_records"] = int(stats["chat_records"]) + 1
    flags = stats["chat_team_flags"]
    assert isinstance(flags, dict)
    key = str(team_flag)
    flags[key] = int(flags.get(key, 0)) + 1
    if zero_field != 0:
        stats["chat_nonzero_dword2"] = int(stats.get("chat_nonzero_dword2", 0)) + 1


def decode_receiver(body: bytes, pos: int) -> tuple[list[int], int]:
    """Decode the receiver field, retaining the file's mixed byte order."""
    if pos >= len(body):
        raise ReplayError("command has no receiver/custom field")
    first = body[pos]
    if first & 0xC0 == 0x40:
        count = first & 0x3F
        pos += 1
    elif first & 0xC0 == 0x80:
        if pos + 2 > len(body):
            raise ReplayError("short two-byte receiver count")
        count = struct.unpack_from(">H", body, pos)[0] & 0x3FFF
        pos += 2
    elif first & 0xC0 == 0xC0:
        if pos + 4 > len(body):
            raise ReplayError("short four-byte receiver count")
        count = struct.unpack_from(">I", body, pos)[0] & 0x3FFFFFFF
        pos += 4
    else:
        if pos + 4 > len(body):
            raise ReplayError("short single receiver")
        return [struct.unpack_from(">I", body, pos)[0]], pos + 4

    end = pos + count * 4
    if end > len(body):
        raise ReplayError("receiver list runs past command")
    return [u32(body, p) for p in range(pos, end, 4)], end


def decode_command(body: bytes) -> dict[str, object]:
    if len(body) < 7:
        raise ReplayError("command body is too short")
    opcode = body[0]
    sender_flags = body[1]
    # body[2:6] is copied/opaque command context in the current evidence.
    receivers, pos = decode_receiver(body, 6)
    custom_type = body[pos]
    pos += 1
    custom_length = 0
    if custom_type != 0xFF:
        if pos >= len(body):
            raise ReplayError("custom field has no length")
        high = body[pos]
        pos += 1
        if high < 0x80:
            custom_length = high
        else:
            if pos >= len(body):
                raise ReplayError("two-byte custom length is truncated")
            custom_length = ((high & 0x7F) << 8) | body[pos]
            pos += 1
        if pos + custom_length != len(body):
            raise ReplayError(
                f"custom payload length {custom_length} does not fit "
                f"remaining {len(body) - pos} bytes"
            )
    elif pos != len(body):
        raise ReplayError("custom type 0xff has an unexpected payload")
    return {
        "opcode": opcode,
        "sender_flags": sender_flags,
        "sender_slot": sender_flags & 0x7F,
        "process_type": (sender_flags >> 7) & 1,
        "receivers": receivers,
        "custom_type": custom_type,
        "custom_length": custom_length,
    }


def parse_sync(payload: bytes, stats: dict[str, object]) -> None:
    # A type-0 payload is a serialized GameMsgSyncCommand.  The reader in the
    # analysed build validates only the high nibble of the kind byte.
    if len(payload) < 17 or payload[0] >> 4 != 2:
        raise ReplayError("sync payload has no kind-2 marker/header")
    tick, command_counter, sync_crc, bundle_count = struct.unpack_from(
        "<IIII", payload, 1
    )
    stats["sync_records"] = int(stats["sync_records"]) + 1
    stats["ticks_max"] = max(int(stats["ticks_max"]), tick)
    stats["command_counter_max"] = max(
        int(stats["command_counter_max"]), command_counter
    )
    if sync_crc == 0:
        stats["sync_crc_zero"] = int(stats["sync_crc_zero"]) + 1
    stats["bundles"] = int(stats["bundles"]) + bundle_count
    pos = 17
    for _ in range(bundle_count):
        if pos + 12 > len(payload):
            raise ReplayError("action bundle header runs past sync payload")
        # The eight bundle header bytes are serialized verbatim and are zero
        # in the whole corpus; count rather than reject a nonzero value.
        if payload[pos : pos + 8] != b"\0" * 8:
            stats["bundle_header_nonzero"] = (
                int(stats["bundle_header_nonzero"]) + 1
            )
        declared = u32(payload, pos + 8)
        pos += 12
        if pos >= len(payload):
            raise ReplayError("action bundle has no length echo")
        echo = payload[pos]
        pos += 1
        if echo != declared & 0xFF:
            raise ReplayError(
                f"bundle length echo 0x{echo:02x} != 0x{declared & 0xff:02x}"
            )
        bundle_end = pos + declared
        if bundle_end > len(payload):
            raise ReplayError("bundle command region runs past sync payload")
        consumed = 0
        while consumed < declared:
            if pos + 2 > bundle_end:
                raise ReplayError("command length runs past sync payload")
            command_length = u16(payload, pos)
            if command_length < 2 or pos + command_length > bundle_end:
                raise ReplayError(f"invalid command length {command_length}")
            command = decode_command(payload[pos + 2 : pos + command_length])
            stats["commands"] = int(stats["commands"]) + 1
            opcodes = stats["opcodes"]
            assert isinstance(opcodes, dict)
            key = str(command["opcode"])
            opcodes[key] = int(opcodes.get(key, 0)) + 1
            pos += command_length
            consumed += command_length
        if consumed != declared:
            raise ReplayError("bundle command lengths do not sum to declaration")
        if pos != bundle_end:
            raise ReplayError("bundle command region has an unconsumed tail")
        # Empty bundles carry a zero byte after their declaration.  Non-empty
        # bundles do not have a per-command terminator; the next bundle starts
        # immediately after the last command.  This is the boundary that the
        # old parser's unconditional seek skipped.
    if pos != len(payload):
        raise ReplayError("sync payload has an unconsumed tail")


def inspect(path: Path) -> dict[str, object]:
    data = path.read_bytes()
    result: dict[str, object] = {
        "file": str(path),
        "size": len(data),
        "header": header(data),
        "chunks": [],
        "players": [],
        "declared_sync_records": 0,
        "sync_records": 0,
        "chat_records": 0,
        "chat_team_flags": {},
        "bundles": 0,
        "bundle_header_nonzero": 0,
        "commands": 0,
        "ticks_max": 0,
        "command_counter_max": 0,
        "sync_crc_zero": 0,
        "opcodes": {},
    }
    # The replay writer uses two Chunky instances on one file, so the 36-byte
    # Relic Chunky v3 signature appears at 84 and again between FOLDPOST and
    # DATASDSC.  Validate both instead of skipping them as opaque gaps.
    chunky_signature(data, 84)
    first = chunk_at(data, 120, len(data))
    if first.name != "FOLDPOST" or first.end != 180:
        raise ReplayError("unsupported Chunky prelude/FOLDPOST layout")
    if not first.children or first.children[0].name != "DATADATA":
        raise ReplayError("FOLDPOST does not contain DATADATA")
    datadata = first.children[0]
    if datadata.size != 4:
        raise ReplayError("FOLDPOST DATADATA is not a single u32")
    # The DATADATA u32 is the sync-record count, patched in when recording is
    # finalized; it is checked against the stream census below.
    result["declared_sync_records"] = u32(data, datadata.offset + 28)
    chunky_signature(data, first.end)
    scenario = chunk_at(data, first.end + 36, len(data))
    if scenario.offset != 216 or scenario.name != "DATASDSC":
        raise ReplayError("unsupported scenario Chunky layout")
    foldinfo = chunk_at(data, scenario.end, len(data))
    if foldinfo.name != "FOLDINFO":
        raise ReplayError(
            f"expected FOLDINFO after scenario chunk, got {foldinfo.name!r}"
        )
    chunks = [first, scenario, foldinfo]
    players = result["players"]
    assert isinstance(players, list)
    for child in foldinfo.children:
        if child.name != "FOLDGPLY":
            continue
        for grandchild in child.children:
            if grandchild.name == "DATAINFO":
                players.append(parse_player_info(data, grandchild))
    stream_start = foldinfo.end
    result["chunks"] = [
        {"name": c.name, "offset": c.offset, "version": c.version, "size": c.size, "end": c.end}
        for c in flatten(chunks)
    ]
    pos = stream_start
    while pos < len(data):
        if pos + 8 > len(data):
            raise ReplayError(f"short outer record at 0x{pos:x}")
        record_type, payload_length = struct.unpack_from("<II", data, pos)
        payload_start = pos + 8
        payload_end = payload_start + payload_length
        if payload_end > len(data):
            raise ReplayError(f"outer record at 0x{pos:x} runs past EOF")
        payload = data[payload_start:payload_end]
        if record_type == 0:
            parse_sync(payload, result)
        elif record_type == 1:
            parse_chat(payload, result)
        else:
            raise ReplayError(f"unknown outer record type {record_type} at 0x{pos:x}")
        pos = payload_end
    if pos != len(data):
        raise ReplayError("record stream does not end at EOF")
    if result["declared_sync_records"] != result["sync_records"]:
        raise ReplayError(
            f"FOLDPOST declares {result['declared_sync_records']} sync records, "
            f"stream contains {result['sync_records']}"
        )
    return result


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("replay", type=Path, nargs="+")
    args = parser.parse_args(argv)
    for path in args.replay:
        try:
            print(json.dumps(inspect(path), sort_keys=True))
        except (OSError, ReplayError, struct.error) as exc:
            print(f"{path}: {exc}", file=sys.stderr)
            return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
