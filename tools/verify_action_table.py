#!/usr/bin/env python3
"""Verify the byte examples in docs/action-table-audit.md.

This is intentionally stdlib-only and does not require a replay file. It
checks the inclusive command length, sender/process split, receiver byte
orders, and custom-type sentinel/length rules used by the documentation.
"""

from __future__ import annotations

import struct


def decode(command: bytes) -> dict:
    if len(command) < 2:
        raise ValueError("missing command length")
    size = struct.unpack_from("<H", command)[0]
    if size != len(command):
        raise ValueError(f"inclusive size {size} != {len(command)}")
    body = command[2:]
    if len(body) < 8:
        raise ValueError("command body is too short")

    opcode, flags = body[:2]
    context = body[2:6]
    offset = 6
    first = body[offset]
    if first & 0xC0 == 0:
        raw_receivers = [int.from_bytes(body[offset : offset + 4], "big")]
        offset += 4
    else:
        if first & 0xC0 == 0x40:
            count = first & 0x3F
            offset += 1
        elif first & 0xC0 == 0x80:
            count = int.from_bytes(body[offset : offset + 2], "big") & 0x3FFF
            offset += 2
        else:
            count = int.from_bytes(body[offset : offset + 4], "big") & 0x3FFFFFFF
            offset += 4
        raw_receivers = []
        for _ in range(count):
            raw_receivers.append(int.from_bytes(body[offset : offset + 4], "little"))
            offset += 4

    custom_type = body[offset]
    offset += 1
    if custom_type == 0xFF:
        if offset != len(body):
            raise ValueError("0xff custom sentinel has payload")
        custom_length = None
        custom_data = b""
    else:
        first_length = body[offset]
        offset += 1
        if first_length < 0x80:
            custom_length = first_length
        else:
            custom_length = ((first_length & 0x7F) << 8) | body[offset]
            offset += 1
        custom_data = body[offset:]
        if len(custom_data) != custom_length:
            raise ValueError("custom length does not consume body")

    return {
        "opcode": opcode,
        "sender": flags & 0x7F,
        "process": flags >> 7,
        "context": context,
        "receivers": raw_receivers,
        "custom_type": custom_type,
        "custom_length": custom_length,
        "custom_data": custom_data,
    }


# Engine names recovered from the analysed build (SCAR registration, .data
# name arrays, literal issuer call sites; see docs/action-table-audit.md).
# Build-scoped: a mod or other game version may renumber these.
OPCODE_NAMES = {
    1: "CMD_Stop",
    2: "CMD_Destroy",
    3: "CMD_BuildSquad",
    5: "CMD_CancelProduction",
    9: "CMD_Attack",
    11: "CMD_RallyPoint",
    13: "CMD_Ability",
    15: "CMD_Upgrade",
    23: "CMD_UnloadSquads",
    43: "SCMD_Stop",
    44: "SCMD_Move",
    47: "SCMD_Capture",
    48: "SCMD_Attack",
    49: "SCMD_ReinforceUnit",
    50: "SCMD_Upgrade",
    51: "SCMD_CancelProduction",
    52: "SCMD_AttackMove",
    53: "SCMD_Ability",
    56: "SCMD_Load",
    58: "SCMD_UnloadSquads",
    61: "SCMD_Retreat",
    70: "SCMD_Melee",
    71: "SCMD_SetWeaponStance",
    78: "PCMD_ConstructStructure",
    85: "PCMD_Ability",
    89: "PCMD_ConstructFence",
    94: "PCMD_Ping",
    96: "PCMD_ResurrectHero",
    98: "PCMD_SetPreferredRetreatTarget",
}

# Packed Target wire sizes on the raw adapter (type byte + variant payload).
TARGET_SIZES = {"none": 1, "squad": 3, "entity": 5, "position": 13}


def check_custom_lengths() -> None:
    """Verify the documented tag layouts reproduce the corpus lengths."""
    t = TARGET_SIZES
    # tag 1 = Target alone
    assert sorted(t.values()) == [1, 3, 5, 13]
    # tag 3 = Target + Target (observed with two positions)
    assert t["position"] * 2 == 26
    # tag 6 = u32 + Target(position)
    assert 4 + t["position"] == 17
    # tag 12 = u16 squad id + 12-byte position
    assert 2 + 12 == 14
    # tags 15/16 = u32 EBP + 2 positions + u8 + entity u32 + squad u16
    assert 4 + 12 + 12 + 1 + 4 + 2 == 35
    # tag 18 = Target(position) + u32; tag 19 = 2 Targets(position) + u32
    assert t["position"] + 4 == 17
    assert t["position"] * 2 + 4 == 30
    # tag 25 = u32 ability id + u8
    assert 4 + 1 == 5
    # tags 26..29 = u32 + 2 flag bytes + 1/2/5/10 packed targets
    head = 4 + 1 + 1
    assert head + t["squad"] == 9 and head + t["entity"] == 11
    assert head + t["position"] == 19
    assert head + 2 * t["position"] == 32
    assert head + t["squad"] + 4 * t["none"] == 13
    assert head + t["position"] + 4 * t["none"] == 23
    assert head + 4 * t["position"] + t["none"] == 59
    assert head + 5 * t["position"] == 71
    assert head + 2 * t["position"] + 8 * t["none"] == 40
    assert head + 6 * t["position"] + 4 * t["none"] == 88
    # tag 39 = 12-byte position + u32
    assert 12 + 4 == 16


check_custom_lengths()

single = bytes.fromhex("12 00 03 03 eb 03 00 00 10 00 77 0c 05 04 86 00 00 00")
a = decode(single)
assert (a["opcode"], a["sender"], a["process"]) == (3, 3, 0)
assert OPCODE_NAMES[a["opcode"]] == "CMD_BuildSquad"
assert struct.unpack("<I", a["custom_data"])[0] == 134  # SBP index in this build
assert a["context"] == bytes.fromhex("eb 03 00 00")
assert a["receivers"] == [0x1000770C]
assert (a["custom_type"], a["custom_length"], a["custom_data"]) == (5, 4, bytes.fromhex("86 00 00 00"))

listed = bytes.fromhex("12 00 44 03 eb 03 00 00 42 34 12 00 10 cd ab 00 20 ff")
b = decode(listed)
assert b["receivers"] == [0x10001234, 0x2000ABCD]
assert (b["custom_type"], b["custom_length"], b["custom_data"]) == (0xFF, None, b"")

extended_data = bytes([0xCD]) * 0x1234
extended_body = bytes([0xFA, 0x81, 0, 0, 0, 0, 0x40, 7, 0x92, 0x34]) + extended_data
extended = struct.pack("<H", len(extended_body) + 2) + extended_body
c = decode(extended)
assert (c["sender"], c["process"]) == (1, 1)
assert (c["custom_type"], c["custom_length"]) == (7, 0x1234)
assert c["custom_data"] == extended_data

print("action-table examples: ok")
