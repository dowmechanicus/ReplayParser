import struct
import unittest

from tools.replay_inspect import ReplayError, parse_chat, parse_sync


def sync_payload(command_size=18, declared=18, trailing=b""):
    # opcode 3, sender 3, reserved bytes eb 03 00 00, one entity receiver,
    # type-5 custom value 134.
    body = bytes.fromhex("03 03 eb 03 00 00 10 00 77 0c 05 04 86 00 00 00")
    command = struct.pack("<H", command_size) + body
    return (
        b"\x20"
        + struct.pack("<IIII", 1, 0, 0, 1)
        + b"\0" * 8
        + struct.pack("<I", declared)
        + bytes([declared & 0xFF])
        + command
        + trailing
    )


def chat_payload(inner_kind=1, inner_delta=8, sender_key=1001, team_flag=1):
    name = "Floid".encode("utf-16le")
    text = "gl hf".encode("utf-16le")
    body = (
        struct.pack("<I", len(name) // 2)
        + name
        + struct.pack("<III", sender_key, 0, team_flag)
        + struct.pack("<I", len(text) // 2)
        + text
    )
    return struct.pack("<II", inner_kind, len(body) + 8 - inner_delta) + body


def fresh_stats():
    return {
        "sync_records": 0,
        "ticks_max": 0,
        "bundles": 0,
        "bundle_header_nonzero": 0,
        "commands": 0,
        "command_counter_max": 0,
        "sync_crc_zero": 0,
        "chat_records": 0,
        "chat_team_flags": {},
        "opcodes": {},
    }


class ReplayInspectorBoundaryTests(unittest.TestCase):
    def test_valid_command(self):
        stats = fresh_stats()
        parse_sync(sync_payload(), stats)
        self.assertEqual(stats["commands"], 1)

    def test_bundle_declaration_cannot_run_past_sync(self):
        with self.assertRaises(ReplayError):
            parse_sync(sync_payload(declared=19), fresh_stats())

    def test_command_cannot_run_past_bundle(self):
        with self.assertRaises(ReplayError):
            parse_sync(sync_payload(command_size=19), fresh_stats())

    def test_sync_tail_is_rejected(self):
        with self.assertRaises(ReplayError):
            parse_sync(sync_payload(trailing=b"\0"), fresh_stats())


class ChatRecordTests(unittest.TestCase):
    def test_valid_chat(self):
        stats = fresh_stats()
        parse_chat(chat_payload(), stats)
        self.assertEqual(stats["chat_records"], 1)
        self.assertEqual(stats["chat_team_flags"], {"1": 1})

    def test_inner_kind_must_duplicate_outer_type(self):
        with self.assertRaises(ReplayError):
            parse_chat(chat_payload(inner_kind=2), fresh_stats())

    def test_inner_size_must_be_payload_minus_eight(self):
        with self.assertRaises(ReplayError):
            parse_chat(chat_payload(inner_delta=9), fresh_stats())

    def test_sender_key_must_be_in_player_namespace(self):
        with self.assertRaises(ReplayError):
            parse_chat(chat_payload(sender_key=7), fresh_stats())
