# Command framing and parser migration

This note records the action framing used by the replay files currently
covered by the parser. It is deliberately separate from the higher-level
replay format notes: it describes the bytes consumed by `parse_action` and the
meaning of the public `Action::data` field.

## Wire layout

The replay tail is a sequence of records. Every record begins with two little-
endian `u32` values:

```text
u32 record_type
u32 payload_bytes
byte payload[payload_bytes]
```

An action record (`record_type == 0`) starts its payload with a tag byte,
followed by the tick number, two other `u32` values, and a `u32` bundle count.
Each bundle is encoded as:

```text
u64 bundle_metadata
u32 command_bytes
u8  command_bytes_low       // redundant low byte of command_bytes
byte command_stream[command_bytes]
```

The command stream is a concatenation of framed commands. A command begins
with a little-endian `u16` size that includes those two size bytes. The
remaining `size - 2` bytes are the command body, beginning with the action
opcode. The bundle count and all lengths are bounds, not hints: a malformed
length causes parsing to stop with `InvalidData` instead of consuming the next
record.

The one-byte bundle value is only an echo of the low byte of the `u32` count.
It must not be used as the stream length. Bundles in the supplied samples
already exceed 255 bytes, and command sizes are still encoded as `u16` values;
the parser must not narrow either field to a one-byte length.

## API changes

`Action::data` now contains the complete command body, including its opcode,
and excludes the two-byte wire size. JSON serialization exposes all of those
bytes. Earlier versions treated a byte before the body as a length, dropped the
last body byte, and serialized only `data[1..20]`. Consumers should therefore:

* read the opcode from `data[0]`;
* treat `data[1..]` as command-specific fields;
* never expect the wire `u16` size in `Action::data`; and
* avoid assuming that the JSON array has a fixed 19-byte maximum.

The public `parse_action` function keeps its signature and still expects a
cursor positioned at an action-record payload. Code that supplied a cursor
containing the outer record type and size must remove those eight bytes first,
or call `parse_replay` instead.

The framing and boundary checks were validated against the repository's five
sample replays. They preserve the existing tick/action/message counts while
retaining adjacent commands and the final byte of each command body.
