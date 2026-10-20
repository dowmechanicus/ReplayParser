# Replay action format

This document describes the binary layout of commands inside a replay file, how the parser turns them into structured output, and how to resolve the numeric IDs they contain into game data names.

> **Correction:** earlier versions of this file described a fixed 19-byte record. That was wrong. Each replay command has an inclusive little-endian `u16` length and a variable-length body.

The old named examples are kept in the [appendix](#appendix-raw-sample-tables) as historical observations. Their labels are hypotheses unless the [audit](docs/action-table-audit.md) explicitly marks the underlying fact as proven.

## Contents

1. [Start here](#1-start-here)
2. [Glossary](#2-glossary)
3. [Command wire format](#3-command-wire-format)
4. [Opcode names](#4-opcode-names)
5. [Which IDs belong to which namespace](#5-which-ids-belong-to-which-namespace)
6. [Parser output](#6-parser-output)
7. [Resolving blueprint names](#7-resolving-blueprint-names---module)
8. [Historical observations](#8-historical-observations)
- [Appendix: raw sample tables](#appendix-raw-sample-tables)

---

## 1. Start here

### Where to find things

| If you want to... | Read |
|---|---|
| See the row-by-row disposition and static evidence for every old action row | [`docs/action-table-audit.md`](docs/action-table-audit.md) |
| Understand the container format and the blueprint, purchase and runtime ID namespaces | [`docs/replay-format.md`](docs/replay-format.md) |
| Learn the RBF, field-name and UCS formats, the archive override rule, and what is verified or hypothesis | [`docs/game-data-formats.md`](docs/game-data-formats.md) |
| Find `Elite.module` without `--module` | [`docs/module-discovery.md`](docs/module-discovery.md) |
| Validate replay boundaries before assigning gameplay meanings | `tools/replay_inspect.py` |

### Quick start

Dump the parser's full structure as JSON, then inspect `.commands`:

```bash
cargo run -p parser -- path/to/replay.rec
```

The replay path is required; a missing path or an unknown argument is a usage error (non-zero exit).
To turn blueprint indices into names, see [section 7](#7-resolving-blueprint-names---module).
For per-player order counts (the `stats` field), see [Replay statistics](#replay-statistics-stats).

### Status words

The document uses these words consistently. Check the word before you trust a claim.

- **Proven**: the audit marks the fact as proven.
- **Verified**: a blueprint command form has been checked against the data (see [section 7](#resolved-command-forms)).
- **Hypothesis**: a working label that is not yet proven.
- **Unresolved**: known to exist, but its meaning has not been established.
- **Opaque**: a custom payload whose tag is outside the recovered set.
- **Historical**: an old observation kept for reference.

---

## 2. Glossary

- **Command**: one framed action in a replay's bundle command stream.
- **Opcode**: the command's type byte (body offset 0). See [section 4](#4-opcode-names).
- **Sender slot**: the player slot that issued the command, taken from the flags byte (`flags & 0x7f`).
- **Process flag**: the high bit of the flags byte. It is the serialized command process flag.
- **Context bytes**: four reserved bytes copied onto the wire verbatim.
- **Receiver**: what the command applies to. It is either a single packed word or a list of them. Observed receiver types are player `0`, entity `1` and squad `2`.
- **Custom payload**: tagged data that follows the receiver. Tag `0xff` means there is none.
- **Runtime instance ID**: the ID of a live object in a match. It is not a blueprint ID.
- **Blueprint**: a game data definition. Commands refer to one by an index into a catalogue.
- **Catalogue**: the ordered list of blueprints of one kind: `ebp`, `sbp`, `upgrade` or `ability`.
- **Ordinal (token)**: a per-producer queue position used for cancellations. It is a runtime token, not a blueprint.
- **`mod_chksum`**: the data checksum stored in the replay. It identifies the build that recorded it.

---

## 3. Command wire format

Every command is framed inside a bundle command stream like this:

```text
u16 command_size                 # little endian, includes these 2 bytes
u8  opcode                       # body offset 0
u8  sender_and_process_flags     # body offset 1
u8[4] reserved_command_context   # body offsets 2..5
... receiver field ...
u8  custom_type                  # 0xff means no custom payload
... custom length and payload ...
```

- `command_size` is little-endian and includes its own 2 bytes.
- The opcode is body offset 0. The flags byte is body offset 1. The four reserved context bytes are body offsets 2 to 5.

### Sender and process flag

The sender slot is `flags & 0x7f`. The high bit is the serialized command process flag.

### Context bytes

The four context bytes are copied onto the wire verbatim. Native `CommandIssueProxy` construction fills them as follows:

- The first `u16` comes from a virtual identity getter.
- The second `u16` comes from a per-proxy 16-bit counter.

Both are set before the call to `WorldCommand::SetReserved`. The getter's exact semantic name remains unresolved.

In the six-file audit corpus:

- `u16_le(context[0..2]) == 1000 + sender` holds for all 30,373 commands.
- The next little-endian `u16` usually increments from zero per sender. The file `recs/1.rec` has one stream reset, from 1037 to zero.
- No counter limit or wrap rule is proven.

### Receiver

- **Single receiver:** a big-endian packed word, `(type << 28) | instance_id`.
- **Receiver list:** a tagged big-endian count (`0x40..`, `0x8000..` or `0xc0000000..`), followed by little-endian packed receiver words.
- **Receiver types observed:** player `0`, entity `1`, squad `2`.
- Runtime instance IDs are not blueprint IDs.

### Custom payload

- Custom lengths below `0x80` occupy one byte. Larger lengths use `((first & 0x7f) << 8) | second`.
- Type `0xff` means there is no custom payload.
- Every custom tag observed in the corpus has a recovered layout. Each layout is built from three primitives and, in construction forms, one more:
  - `u32`
  - a 12-byte position
  - a packed **Target**: `u8 type`, followed by nothing, a `u16` squad, a `u32` entity or a 12-byte position. Wire sizes are 1, 3, 5 and 13 bytes.
  - in construction forms, a packed builder pair of `u32` entity and `u16` squad.
- The custom-tag codec table is in the audit. Tags outside the recovered set stay opaque.

---

## 4. Opcode names

For the analysed build, the opcode byte also has an engine name. The executable records the command enum in three places, which agree wherever they overlap:

1. the SCAR constant registration;
2. three name-pointer arrays: `CMD_*` (base 0), `SCMD_*` (base 41) and `PCMD_*` (base 78);
3. the literal opcodes passed to the `WorldDoCommand*` issuers.

These names are scoped to the analysed build. They are not a contract across versions. The six-file corpus observes 27 opcode bytes, and all of them have engine names recovered from this build. The [audit](docs/action-table-audit.md) has every old row, its observed custom shapes, the enum evidence and its disposition.

| opcode | engine name | opcode | engine name | opcode | engine name |
| ---: | --- | ---: | --- | ---: | --- |
| 2 | `CMD_Destroy` | 44 | `SCMD_Move` | 58 | `SCMD_UnloadSquads` |
| 3 | `CMD_BuildSquad` | 47 | `SCMD_Capture` | 61 | `SCMD_Retreat` |
| 5 | `CMD_CancelProduction` | 48 | `SCMD_Attack` | 70 | `SCMD_Melee` |
| 9 | `CMD_Attack` | 49 | `SCMD_ReinforceUnit` | 71 | `SCMD_SetWeaponStance` |
| 11 | `CMD_RallyPoint` | 50 | `SCMD_Upgrade` | 78 | `PCMD_ConstructStructure` |
| 13 | `CMD_Ability` | 51 | `SCMD_CancelProduction` | 85 | `PCMD_Ability` |
| 15 | `CMD_Upgrade` | 52 | `SCMD_AttackMove` | 89 | `PCMD_ConstructFence` |
| 23 | `CMD_UnloadSquads` | 53 | `SCMD_Ability` | 94 | `PCMD_Ping` |
| 43 | `SCMD_Stop` | 56 | `SCMD_Load` | 96 | `PCMD_ResurrectHero` |

### What the type-5 values mean

The type-5, length-4 `u32` values resolve per opcode through `PropertyBagGroupManager::GetGroup(type, id)`:

| Opcode | The `u32` is an index into |
|---|---|
| 3 `CMD_BuildSquad` | SBP (squad blueprints) |
| 15 `CMD_Upgrade`, 50 `SCMD_Upgrade` | `Upgrade\` |
| 49 `SCMD_ReinforceUnit` | EBP (entity blueprints) |
| 13, 53, 85 (tag-25 ability forms) | `Ability\` |
| 78, 89, 90 (35-byte form) | the EBP index of the structure, which the form leads with |

- **Cancellations** (opcodes 5 and 51) carry 1-based per-producer enqueue ordinals. These are runtime tokens, not blueprints.
- **Blueprint indices** are per-type catalogue positions in the build that recorded the replay. RBF builds assign them by case-insensitive (`_stricmp`) path order. RB2 builds assign them by entry order. Raw values can't be compared across versions unless they are keyed by the header checksum.

---

## 5. Which IDs belong to which namespace

The old fixed rows are not a cross-version ID catalogue. These values live in different namespaces:

- receiver instance IDs;
- purchase or queue values;
- attribute blueprint and group indices.

A replay records numeric references and a data checksum. It does not embed the complete mod attribute archive. To match a number to a unit name across game or mod versions, you need the matching data and a reconstruction of that session's runtime group creation order.

---

## 6. Parser output

- **`ReplayInfo.commands`** is the complete structural view. Use this one.
- **`ReplayInfo.actions`** is an older view. It is filtered for compatibility and omits some opcode values.

### Replay statistics (`stats`)

Every parse fills `ReplayInfo.stats`, so the field is always present in the JSON dump. There is no flag, and no game files or `--module` are needed. It is a JSON array with one object per player, sorted by slot.

```bash
cargo run -q -p parser -- path/to/replay.rec | jq .stats
```

Each object has the identity fields `slot`, `name`, `relic_id` and `race`, plus these counts:

| field | counts |
|---|---|
| `actions` | every decoded command, including opcodes with no engine name such as 97 (counted, not interpreted) |
| `squads_built` | opcode 3 `CMD_BuildSquad` |
| `retreats` | opcode 61 `SCMD_Retreat` |
| `reinforcements` | opcode 49 `SCMD_ReinforceUnit` |
| `hero_resurrections` | opcode 96 `PCMD_ResurrectHero` |
| `point_captures` | opcode 47 `SCMD_Capture` |
| `cancels` | opcode 5 `CMD_CancelProduction` plus opcode 51 `SCMD_CancelProduction` |

Caveats:

- All counts are orders issued, not outcomes. A retreat, capture or reinforce order does not prove it happened.
- `squads_built` includes orders that were later cancelled. Opcodes 5 and 51 carry per-producer queue ordinals ([section 4](#4-opcode-names)), so cancels cannot be matched to builds. `cancels` is reported separately and covers all producers, not just squads.
- No opcode in the named set is a hero-death command. `hero_resurrections` is only a proxy, and it misses deaths that were never resurrected.
- `point_captures` is one aggregate. The capture target is a runtime entity ID (custom tag 1, length 5, `[3, u32]`, where the first byte 3 is the entity target kind). No field in the command distinguishes requisition points from power points. The split is unresolved.
- A command whose sender slot matches no seated player is reported in an extra entry with null `name`, `relic_id` and `race`. Players without an assigned slot get no entry of their own.
- Counts come from `ReplayInfo.commands`, not the legacy `actions` view.

Real `stats` output for `1v1.rec`, a local sample that is not committed to the repo:

```json
[
  {"slot": 0, "name": "VindicareX", "relic_id": 10203601, "race": "space_marines",
   "actions": 4323, "squads_built": 8, "retreats": 90, "reinforcements": 28,
   "hero_resurrections": 0, "point_captures": 218, "cancels": 2},
  {"slot": 1, "name": "^505LavrKornilov", "relic_id": 12370470, "race": "imperial_guard",
   "actions": 4162, "squads_built": 7, "retreats": 82, "reinforcements": 67,
   "hero_resurrections": 4, "point_captures": 234, "cancels": 5}
]
```

The tool prints the same data in indented form. The layout above is condensed.

---

## 7. Resolving blueprint names (`--module`)

### Running it

Pass the install's module file to turn blueprint indices into names:

```text
cargo run -q -p parser -- temp.rec --module "<install>/Elite.module"
```

Without `--module`, the parser tries to find `Elite.module` itself, in this order (details and
verification status in [`docs/module-discovery.md`](docs/module-discovery.md)):

1. `DOW2_INSTALL`, when set: `Elite.module` directly in that directory;
2. Steam (via the `steamlocate` crate): Retribution (app 56400) first, then the Anniversary
   Edition (15620), then a scan of every library's `steamapps\common\*\Elite.module`.

An app-id match wins over a scan match. Only *installed* apps are seen (steamlocate reads
`appmanifest_*.acf`). When several candidates are found, the first by that order is used and
stderr lists them all. When none is found, the parse continues without names and stderr says
so. An auto-located module behaves exactly like a `--module` argument: its path lands in
`ReplayInfo.blueprints.module`, and the install dir is the module's parent.

### How catalogues are built

The parser reads the module's `[attrib:common]` SGA archives and builds four catalogues:

- `ebp` (`simulation\attrib\ebps\`)
- `sbp` (`sbps\`)
- `upgrade`
- `ability`

The rule: every `.rbf` path under that directory, across all the archives, is lowercased, deduplicated and byte-sorted. Index *i* is the *i*-th path.

Two parts of this rule are not discriminated by the data:

- Including `Elite_Campaign_Attrib.sga` is a hypothesis.
- Whether directories that sort after `ebps\` belong to the EBP namespace is open. This affects only the EBP count. See `docs/replay-format.md`.

For the build with `mod_chksum` 4156674300 (`0xf7c1d0fc`), the counts are: ebp 2591, sbp 715, upgrade 1751, ability 2399.

### Resolved command forms

Each command whose (opcode, custom tag, length) is a verified blueprint form gets a `blueprint` object:

`command.blueprint = {"kind","index","name","path","display_name"}`

- `name` is the file stem.
- `path` is relative to the namespace directory. Basenames can collide across directories, so use `path` when in doubt.
- `display_name` is the readable name. See [Display names](#display-names).

| opcode | form | kind | status |
|---|---|---|---|
| 3 `CMD_BuildSquad` | tag 5, len 4 | `sbp` | verified |
| 15 `CMD_Upgrade`, 50 `SCMD_Upgrade` | tag 5, len 4 | `upgrade` | verified |
| 49 `SCMD_ReinforceUnit` | tag 5, len 4 | `ebp` | verified |
| 78 `PCMD_ConstructStructure`, 89 `PCMD_ConstructFence` | tag 15 / 16, len 35, leading `u32` | `ebp` | verified (89: only 3 commands seen) |
| 53 `SCMD_Ability`, 85 `PCMD_Ability` | tags 25..29, len >= 5, leading `u32` | `ability` | 53 verified for tags 25 and 26; 85 verified for tags 25, 26, 28 and 29 (the resolver accepts 25..=29) |


Global (commander) abilities are opcode 85 and resolve into `ability\pvp\race_*\globals\`. There is no separate commander namespace.

### Deliberately not resolved

- Opcode 13 (`CMD_Ability`): unverified.
- Opcodes 90, and 4, 16, 63 and 64: documented only.
- Opcodes 5 and 51: queue ordinals.
- Opcode 71: stance.
- The tag-1 forms of 53 and 85.
- Opcodes 94 and 96: not blueprints.

### Blueprint metadata

`ReplayInfo.blueprints` records the `module` and the catalogue sizes (`ebp_count`, `sbp_count`, `upgrade_count`, `ability_count`).

With `--module`, names are always resolved. There is no build check.

### Example output

Real output for `temp.rec` (`ReplayInfo.blueprints`, then two commands, trimmed):

```json
"blueprints": {"module": ".../Elite.module",
  "ebp_count": 2591, "sbp_count": 715, "upgrade_count": 1751, "ability_count": 2399}

{"opcode": 78, "opcode_name": "PCMD_ConstructStructure", "sender_slot": 4,
 "custom": {"tag": 15, "length": 35, "data": [174, 1, 0, 0, ...]},
 "blueprint": {"kind": "ebp", "index": 430, "name": "csm_listening_post",
   "path": "pvp\\race_chaos\\buildings\\csm_listening_post",
   "display_name": "Chaos Power Node"}}

{"opcode": 85, "opcode_name": "PCMD_Ability", "sender_slot": 5,
 "custom": {"tag": 26, "length": 19, "data": [34, 3, 0, 0, ...]},
 "blueprint": {"kind": "ability", "index": 802, "name": "ig_spawn_stormtroopers",
   "path": "pvp\\race_imperial_guard\\globals\\ig_spawn_stormtroopers",
   "display_name": "Infiltrated Storm Troopers"}}
```

### Build mismatch

Indices are specific to a build. If you resolve a replay recorded on a different build from the installed one:

- You get wrong names that are still in range, and nothing warns you. In the 2015 `3v3.rec`, the Imperial Guard player "builds" `gk_*` squads.
- The replay's `mod_chksum` is part of the output, so you can compare builds yourself.
- `mod_chksum` cannot yet be computed from the archives. The value 4156674300 is linked to the analysed install by data fit, not by derivation.

### Display names

With `--module`, each resolved `blueprint` also gets a `display_name`:

1. Take the blueprint's `screen_name`, which is a localisation ID stored in its RBF attribute file.
2. Look it up in `DOW2.ucs` plus `<module name>.ucs` (`Elite.ucs`), both from `GameAssets\Locale\English\`.
3. Trim the text.

Only English is supported, and there is no language flag.

For `temp.rec`, all 1014 `blueprint` objects (counting `commands` and `actions`) have a `display_name`, with empty stderr. The 166 distinct blueprints in that replay all resolve.

**When `display_name` is missing.** The key is absent, not `null`, when the blueprint has no name. That happens if it has no `screen_name` field, its ID is 0, its ID is missing from the UCS files, or its UCS text is blank. `name` and `path` are always present.

**Warnings (stderr).** Neither warning stops the parse.

- `warning: display names unavailable: <error>` is printed once if the name data cannot be loaded. Causes include:
  - `fieldnames.flb`, `DOW2.ucs` or the locale folder is missing;
  - a needed field name is missing;
  - `fieldnames.flb`, `DOW2.ucs` or `<Name>.ucs` is malformed. A corrupt `<Name>.ucs` drops all names.

  No `display_name` is written in this case.
- `warning: no display name for <kind> <path>: <error>` is printed once when a blueprint's RBF is missing or malformed. That blueprint is left without a name.

**Caveats.**

- **Names are not unique.** "Repair" belongs to three blueprints. Use `path` as the identity.
- **Reinforce resolves to the model entity.** Opcode 49 resolves to the *model* entity. Some models carry another unit's string: the EBP `sm_sternguard_veteran` shows "Tactical Marine". So the name can differ from the squad that was reinforced.
- **Other builds.** On a replay from a different build, display names are as wrong as the indices. They look more convincing than a file stem, so compare `mod_chksum` before trusting either.

### Extending the handler chain

Post-parse processing in `parser_lib::pipeline` is a chain of handlers. Implement
`ReplayHandler` (`fn handle(&self, replay: &mut ReplayInfo, ctx: &Context) -> Result<(), HandlerError>`;
the trait requires `Send + Sync`) and append it to the built-in chain:

```rust
let game = GameData::open(&module_path)?;           // or None when no module is available
let chain = Chain::with_defaults(Some(&game)).with(MyHandler);
chain.run(&mut replay, Some(&game))?;
```

- `with_defaults` registers blueprint resolution, then display names, so those run first. With
  `None` it registers nothing. Handlers added with `.with()` or `.push()` run after, in
  registration order.
- `HandlerError::Warn(msg)` prints `warning: <msg>` to stderr and the chain continues.
  `HandlerError::Fatal(msg)` stops the chain and `run` returns `Err(msg)`.
- `ctx.game()` is `None` when the chain runs without a module, so handlers that need game
  files must check it. The chain still runs in that case.

---

## 8. Historical observations

The bullets in this section record patterns reported from a small sample. They are not confirmed wire-field meanings. In particular, pairs such as `(25, 5)` are observed custom type/length shapes. They do not globally identify an ability category, a target kind or a unit ID.

- Abilities that can be toggled are identical apart from the action counter. As far as the game is concerned you have
  used the same ability twice.

- Targettable abilities seem to also have location information stored in them (probably origin and target coordinates)

- Unit id (once unit is on the field) is not the same as the unit/item id when its purchased at the HQ

- Action data fields 12 + 13 seem to decode the following (amongst other stuff):
  - (1, 3) => Something to do with action type 48; also seen with action type 23
  - (1, 5) => Capturing
  - (1, 13) => Movement
  - (3, 26) => Movement + Direction
  - (5, 4) => Purchase
  - (6, 17) => Movement
  - (12, 4) => Unknown (in combination with action type 96)
  - (12, 14) => Unknown (in combination with action type 96)
  - (18, 17) => Movement
  - (15, 35) => Single target, building (e.g. Webway Gate)
  - (25, 5) => Non-targettable, affects every target-type at once (e.g. Angels of Death, Blessing of the Omnissiah, Swift Movement) or is called in at base (e.g. Seer Council). Also applicable to unit abilities that are self-targetting
  - (26, 9) => Single target, unit or global ability (e.g. Scout grenades, Farseer Guide, For the Emperor, Crackshot)
  - (26, 19) => Single target with no secondary projectiles (e.g. Drop Pod, Terminators, Eldritch Storm)
  - (28, 23) => Unknown (in combination with action type 85)
  - (28, 59) => Multiple targets but no unit call-in (e.g. Orbital Bombardement)
  - (28, 71) => Multiple targets and unit call-in (e.g. Autarch)
  - (32, 84) => Something to do with action type 48
  - (39, 16) => Unknown (in combination with action type 94)

- Canceling a unit purchase functions like clearing an item from an array without removing the item itself ("nulling" instead of delete). There exists an internal counter that keeps track of each distinct purchase
  even if the same unit is purchases and immediately cancelled. That will still cause the counter to increment.

---

## Appendix: raw sample tables

### Global Abilities

The tables in this appendix are preserved raw and historical examples. Their column headings follow the current wire layout (see [section 3](#3-command-wire-format)). The leading `0` is read as the high byte of the `u16` command size. The samples appear to have been captured one byte late, so they omit the size low byte and the command's final byte. A row therefore normally shows one payload byte fewer than the declared length. The last column counts payload bytes present against the declared length. Columns marked "(decoded)" are derived from the raw bytes and are not part of the samples. Some rows contain a complete variable command tail and some are incomplete. The names and numeric IDs are historical, build-specific observations and not a cross-version ID map. Use the corrected layout (section 3) and the audit when interpreting these bytes.



#### Space Marines (Techmarine, Apothecary, Force Commander)

| Name | size (hi byte) | opcode | flags | context | receiver | tag | length | payload | sender / process (decoded) | player id / counter (decoded) | receiver (decoded) | payload bytes (decoded) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Blessing of the Omnissiah | 0 | 85 | 1 | 233 3 0 0 | 0 0 3 233 | 25 | 5 | 115 3 0 0 | 1 / no | 1001 / 0 | player 1001 | 4 of 5 |
| Drop Pod | 0 | 85 | 1 | 233 3 1 0 | 0 0 3 233 | 26 | 19 | 123 3 0 0 1 0 2 74 75 68 193 72 5 210 66 126 168 37 | 1 / no | 1001 / 1 | player 1001 | 18 of 19 |
| Venerable Dreadnought | 0 | 85 | 1 | 233 3 4 0 | 0 0 3 233 | 26 | 19 | 125 3 0 0 1 0 2 90 174 28 65 51 243 209 66 100 222 42 | 1 / no | 1001 / 4 | player 1001 | 18 of 19 |
| Terminator Call-in | 0 | 85 | 1 | 233 3 5 0 | 0 0 3 233 | 26 | 19 | 124 3 0 0 1 0 2 152 31 118 64 96 111 208 66 65 114 31 | 1 / no | 1001 / 5 | player 1001 | 18 of 19 |
| Orbital Bombardement | 0 | 85 | 1 | 233 3 6 0 | 0 0 3 233 | 28 | 59 | 119 3 0 0 1 0 2 12 192 220 192 58 250 209 66 229 136 254 66 2 198 116 30 65 54 139 208 66 232 162 242 66 2 128 5 29 62 113 165 203 66 228 233 217 66 2 77 154 140 193 230 2 204 66 148 187 239 66 | 1 / no | 1001 / 6 | player 1001 | 58 of 59 |
| Larramans Blessing | 0 | 85 | 1 | 233 3 2 0 | 0 0 3 233 | 25 | 5 | 121 3 0 0 | 1 / no | 1001 / 2 | player 1001 | 4 of 5 |
| Drop Pod | 0 | 85 | 1 | 233 3 3 0 | 0 0 3 233 | 26 | 19 | 123 3 0 0 1 0 2 248 94 99 193 16 29 210 66 206 62 38 | 1 / no | 1001 / 3 | player 1001 | 18 of 19 |
| Angels of Death | 0 | 85 | 1 | 233 3 4 0 | 0 0 3 233 | 25 | 5 | 114 3 0 0 | 1 / no | 1001 / 4 | player 1001 | 4 of 5 |
| Terminator Call-in | 0 | 85 | 1 | 233 3 5 0 | 0 0 3 233 | 26 | 19 | 124 3 0 0 1 0 2 42 238 65 65 50 243 209 66 67 252 40 | 1 / no | 1001 / 5 | player 1001 | 18 of 19 |
| Orbital Bombardement | 0 | 85 | 1 | 233 3 6 0 | 0 0 3 233 | 28 | 59 | 119 3 59 119 3 0 0 1 0 2 0 30 115 189 22 77 209 66 24 59 4 67 2 0 30 115 189 22 77 209 66 24 59 4 67 2 0 189 54 63 9 21 209 66 117 205 3 67 2 0 252 45 189 22 77 209 66 159 129 4 67 | 1 / no | 1001 / 6 | player 1001 | 61 of 59 (deviates: over length) |
| For the Emperor | 0 | 85 | 0 | 232 3 2 0 | 0 0 3 232 | 26 | 9 | 117 3 0 0 1 0 4 84 | 0 / no | 1000 / 2 | player 1000 | 8 of 9 |
| Drop Pod | 0 | 85 | 0 | 232 3 3 0 | 0 0 3 232 | 26 | 19 | 123 3 0 0 1 0 2 16 16 2 194 51 220 210 66 101 95 44 | 0 / no | 1000 / 3 | player 1000 | 18 of 19 |
| Assault Terminators | 0 | 85 | 0 | 232 3 4 0 | 0 0 3 232 | 26 | 19 | 122 3 0 0 1 0 2 174 243 136 193 41 220 210 66 39 120 41 | 0 / no | 1000 / 4 | player 1000 | 18 of 19 |
| Ranged Terminators | 0 | 85 | 0 | 232 3 21 0 | 0 0 3 232 | 26 | 19 | 124 3 0 0 1 0 2 52 152 37 193 0 0 180 66 214 222 150 | 0 / no | 1000 / 21 | player 1000 | 18 of 19 |
| Orbital Bombardement | 0 | 85 | 0 | 232 3 23 0 | 0 0 3 232 | 28 | 59 | 119 3 0 0 1 0 2 182 98 104 66 44 185 191 66 146 253 30 194 2 182 98 104 66 44 185 191 66 146 253 30 194 2 182 98 104 66 44 185 191 66 146 253 30 194 2 151 124 104 66 10 236 191 66 118 23 31 194 | 0 / no | 1000 / 23 | player 1000 | 58 of 59 |

#### Eldar (Warlock, WSE, Farseer)

| Name | size (hi byte) | opcode | flags | context | receiver | tag | length | payload | sender / process (decoded) | player id / counter (decoded) | receiver (decoded) | payload bytes (decoded) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Webway Gate | 0 | 78 | 1 | 233 3 2 0 | 0 0 3 233 | 15 | 35 | 198 1 0 0 54 10 136 193 86 236 209 66 211 70 37 67 54 10 136 193 86 236 209 66 211 70 38 67 0 0 0 0 0 0 | 1 / no | 1001 / 2 | player 1001 | 34 of 35 |
| Swift Movement | 0 | 85 | 1 | 233 3 3 0 | 0 0 3 233 | 25 | 5 | 197 1 0 0 | 1 / no | 1001 / 3 | player 1001 | 4 of 5 |
| Distort Field | 0 | 85 | 1 | 233 3 4 0 | 0 0 3 233 | 26 | 9 | 199 1 0 0 1 0 4 86 | 1 / no | 1001 / 4 | player 1001 | 8 of 9 |
| Autarch Drop | 0 | 85 | 1 | 233 3 5 0 | 0 0 3 233 | 28 | 71 | 205 1 0 0 1 0 2 132 130 77 65 65 230 209 66 238 79 38 67 2 96 247 232 64 204 11 209 66 185 40 34 67 2 216 235 88 64 142 157 208 66 251 182 32 67 2 64 188 88 191 192 157 208 66 189 175 32 67 2 28 89 150 192 140 230 208 66 252 39 34 | 1 / no | 1001 / 5 | player 1001 | 70 of 71 |
| Eldritch Storm | 0 | 85 | 1 | 233 3 6 0 | 0 0 3 233 | 26 | 19 | 201 1 0 0 1 0 2 220 244 156 64 120 27 208 66 17 190 12 | 1 / no | 1001 / 6 | player 1001 | 18 of 19 |
| Webway Gate | 0 | 78 | 1 | 233 3 2 0 | 0 0 3 233 | 15 | 35 | 198 1 0 0 250 168 131 193 154 236 209 66 95 52 38 67 250 168 131 193 154 236 209 66 95 52 39 67 0 0 0 0 0 0 | 1 / no | 1001 / 2 | player 1001 | 34 of 35 |
| Crackshot | 0 | 85 | 1 | 233 3 3 0 | 0 0 3 233 | 26 | 9 | 198 1 0 0 1 0 4 87 | 1 / no | 1001 / 3 | player 1001 | 8 of 9 |
| Warpspider Call-in | 0 | 85 | 1 | 233 3 4 0 | 0 0 3 233 | 26 | 19 | 208 1 0 0 1 0 2 144 136 76 65 106 233 209 66 190 147 38 | 1 / no | 1001 / 4 | player 1001 | 18 of 19 |
| Autarch | 0 | 85 | 1 | 233 3 5 0 | 0 0 3 233 | 28 | 71 | 205 1 0 0 1 0 2 24 81 206 64 246 134 209 134 209 66 36 129 35 67 2 240 170 246 64 61 37 208 66 248 46 25 67 2 144 25 23 64 71 32 208 66 19 81 23 67 2 240 153 43 192 2 64 208 66 128 75 24 67 2 40 119 247 192 6 171 208 66 136 140 29 | 1 / no | 1001 / 5 | player 1001 | 72 of 71 (deviates: over length) |
| Eldritch Storm | 0 | 85 | 1 | 233 3 6 0 | 0 0 3 233 | 26 | 19 | 201 1 0 0 1 0 2 56 143 35 64 169 139 205 66 18 196 246 | 1 / no | 1001 / 6 | player 1001 | 18 of 19 |
| Webway Gate | 0 | 78 | 0 | 232 3 2 0 | 0 0 3 232 | 15 | 35 | 198 1 0 0 112 54 26 194 42 220 210 66 14 122 41 195 112 54 26 194 42 220 210 66 14 122 40 195 0 0 0 0 0 0 | 0 / no | 1000 / 2 | player 1000 | 34 of 35 |
| Farsight | 0 | 85 | 0 | 232 3 3 0 | 0 0 3 232 | 26 | 19 | 204 1 0 0 1 0 2 0 184 204 62 157 240 208 66 72 225 34 | 0 / no | 1000 / 3 | player 1000 | 18 of 19 |
| Autarch | 0 | 85 | 0 | 232 3 4 0 | 0 0 3 232 | 28 | 71 | 206 1 0 0 1 0 2 226 10 170 193 22 220 210 66 20 45 31 195 2 41 16 137 193 47 220 210 66 255 99 30 195 2 154 28 118 193 250 219 210 66 85 172 29 195 2 104 196 58 193 218 219 210 66 150 131 31 195 2 56 81 55 193 201 219 210 66 228 109 31 | 0 / no | 1000 / 4 | player 1000 | 70 of 71 |
| Seer Council | 0 | 85 | 0 | 232 3 5 0 | 0 0 3 232 | 25 | 5 | 207 1 0 0 | 0 / no | 1000 / 5 | player 1000 | 4 of 5 |
| Eldritch Storm | 0 | 85 | 0 | 232 3 6 0 | 0 0 3 232 | 26 | 19 | 201 1 0 0 1 0 2 248 246 73 193 55 132 207 66 226 131 6 | 0 / no | 1000 / 6 | player 1000 | 18 of 19 |

### Unit Abilities
#### Space Marines
| Name | size (hi byte) | opcode | flags | context | receiver | tag | length | payload | sender / process (decoded) | player id / counter (decoded) | receiver (decoded) | payload bytes (decoded) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Tacs activate Kraken Rounds | 0 | 53 | 1 | 233 3 6 0 | 32 0 195 85 | 25 | 5 | 164 3 0 0 | 1 / no | 1001 / 6 | squad 0x00c355 | 4 of 5 |
| Scouts cloaking | 0 | 53 | 1 | 233 3 7 0 | 32 0 195 83 | 25 | 5 | 143 3 0 0 | 1 / no | 1001 / 7 | squad 0x00c353 | 4 of 5 |
| Scouts uncloaking | 0 | 53 | 1 | 233 3 8 0 | 32 0 195 83 | 25 | 5 | 143 3 0 0 | 1 / no | 1001 / 8 | squad 0x00c353 | 4 of 5 |
| Scouts throw grenade | 0 | 53 | 1 | 233 3 9 0 | 32 0 195 83 | 26 | 19 | 144 3 0 0 1 0 2 59 226 160 66 0 0 200 66 120 215 161 | 1 / no | 1001 / 9 | squad 0x00c353 | 18 of 19 |
| FC battlecry | 0 | 53 | 1 | 233 3 10 0 | 32 0 195 82 | 25 | 5 | 107 3 0 0 | 1 / no | 1001 / 10 | squad 0x00c352 | 4 of 5 |
| FC alacrity | 0 | 53 | 1 | 233 3 11 0 | 32 0 195 82 | 25 | 5 | 110 3 0 0 | 1 / no | 1001 / 11 | squad 0x00c352 | 4 of 5 |
| FC toggle on halo | 0 | 53 | 1 | 233 3 12 0 | 32 0 195 82 | 25 | 5 | 109 3 0 0 | 1 / no | 1001 / 12 | squad 0x00c352 | 4 of 5 |
| FC toggle off halo | 0 | 53 | 1 | 233 3 13 0 | 32 0 195 82 | 25 | 5 | 109 3 0 0 | 1 / no | 1001 / 13 | squad 0x00c352 | 4 of 5 |

#### Eldar
| Name | size (hi byte) | opcode | flags | context | receiver | tag | length | payload | sender / process (decoded) | player id / counter (decoded) | receiver (decoded) | payload bytes (decoded) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Farseer Fleet of Foot | 0 | 53 | 1 | 233 3 8 0 | 32 0 195 86 | 25 | 5 | 173 1 0 0 | 1 / no | 1001 / 8 | squad 0x00c356 | 4 of 5 |
| Farseer Guide | 0 | 53 | 1 | 233 3 9 0 | 32 0 195 86 | 26 | 9 | 182 1 0 0 1 0 4 87 | 1 / no | 1001 / 9 | squad 0x00c356 | 8 of 9 |

### Unit Purchases
#### Eldar
| Name | size (hi byte) | opcode | flags | context | receiver | tag | length | payload | sender / process (decoded) | player id / counter (decoded) | receiver (decoded) | payload bytes (decoded) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Dire Avengers | 0 | 3 | 1 | 233 3 0 0 | 16 0 120 180 | 5 | 4 | 141 0 0 | 1 / no | 1001 / 0 | entity 0x0078b4 | 3 of 4 |
| Howling Banshees | 0 | 3 | 1 | 233 3 1 0 | 16 0 120 180 | 5 | 4 | 137 0 0 | 1 / no | 1001 / 1 | entity 0x0078b4 | 3 of 4 |
| Rangers | 0 | 3 | 1 | 233 3 2 0 | 16 0 120 180 | 5 | 4 | 146 0 0 | 1 / no | 1001 / 2 | entity 0x0078b4 | 3 of 4 |
| Shuriken Platform | 0 | 3 | 1 | 233 3 3 0 | 16 0 120 180 | 5 | 4 | 144 0 0 | 1 / no | 1001 / 3 | entity 0x0078b4 | 3 of 4 |
| Dark Reapers | 0 | 3 | 1 | 233 3 5 0 | 16 0 120 180 | 5 | 4 | 138 0 0 | 1 / no | 1001 / 5 | entity 0x0078b4 | 3 of 4 |
| Warp Spiders | 0 | 3 | 1 | 233 3 6 0 | 16 0 120 180 | 5 | 4 | 148 0 0 | 1 / no | 1001 / 6 | entity 0x0078b4 | 3 of 4 |
| Fire Dragons | 0 | 3 | 1 | 233 3 7 0 | 16 0 120 180 | 5 | 4 | 140 0 0 | 1 / no | 1001 / 7 | entity 0x0078b4 | 3 of 4 |
| Wraithlord | 0 | 3 | 1 | 233 3 8 0 | 16 0 120 180 | 5 | 4 | 153 0 0 | 1 / no | 1001 / 8 | entity 0x0078b4 | 3 of 4 |
| Falcon | 0 | 3 | 0 | 232 3 1 0 | 16 0 120 173 | 5 | 4 | 151 0 0 | 0 / no | 1000 / 1 | entity 0x0078ad | 3 of 4 |
| Wraithguard | 0 | 3 | 0 | 232 3 2 0 | 16 0 120 173 | 5 | 4 | 149 0 0 | 0 / no | 1000 / 2 | entity 0x0078ad | 3 of 4 |
| Fire Prism | 0 | 3 | 0 | 232 3 4 0 | 16 0 120 173 | 5 | 4 | 152 0 0 | 0 / no | 1000 / 4 | entity 0x0078ad | 3 of 4 |
| D-Cannon | 0 | 3 | 0 | 232 3 5 0 | 16 0 120 173 | 5 | 4 | 143 0 0 | 0 / no | 1000 / 5 | entity 0x0078ad | 3 of 4 |
| Avatar | 0 | 3 | 0 | 232 3 6 0 | 16 0 120 173 | 5 | 4 | 145 0 0 | 0 / no | 1000 / 6 | entity 0x0078ad | 3 of 4 |
| Seer Council | 0 | 3 | 0 | 232 3 7 0 | 16 0 120 173 | 5 | 4 | 147 0 0 | 0 / no | 1000 / 7 | entity 0x0078ad | 3 of 4 |

#### Space Marines
| Name | size (hi byte) | opcode | flags | context | receiver | tag | length | payload | sender / process (decoded) | player id / counter (decoded) | receiver (decoded) | payload bytes (decoded) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Scouts | 0 | 3 | 0 | 232 3 0 0 | 16 0 120 173 | 5 | 4 | 219 0 0 | 0 / no | 1000 / 0 | entity 0x0078ad | 3 of 4 |
| Tactical Marines | 0 | 3 | 0 | 232 3 1 0 | 16 0 120 173 | 5 | 4 | 221 0 0 | 0 / no | 1000 / 1 | entity 0x0078ad | 3 of 4 |
| Devastators | 0 | 3 | 0 | 232 3 2 0 | 16 0 120 173 | 5 | 4 | 216 0 0 | 0 / no | 1000 / 2 | entity 0x0078ad | 3 of 4 |
| Assault Marines | 0 | 3 | 0 | 232 3 3 0 | 16 0 120 173 | 5 | 4 | 215 0 0 | 0 / no | 1000 / 3 | entity 0x0078ad | 3 of 4 |
| Plasma Devastators | 0 | 3 | 0 | 232 3 9 0 | 16 0 120 173 | 5 | 4 | 217 0 0 | 0 / no | 1000 / 9 | entity 0x0078ad | 3 of 4 |
| Dreadnought | 0 | 3 | 0 | 232 3 10 0 | 16 0 120 173 | 5 | 4 | 225 0 0 | 0 / no | 1000 / 10 | entity 0x0078ad | 3 of 4 |
| Razorback | 0 | 3 | 0 | 232 3 11 0 | 16 0 120 173 | 5 | 4 | 228 0 0 | 0 / no | 1000 / 11 | entity 0x0078ad | 3 of 4 |
| Librarian | 0 | 3 | 0 | 232 3 12 0 | 16 0 120 173 | 5 | 4 | 218 0 0 | 0 / no | 1000 / 12 | entity 0x0078ad | 3 of 4 |
| Whirlwind | 0 | 3 | 0 | 232 3 17 0 | 16 0 120 173 | 5 | 4 | 230 0 0 | 0 / no | 1000 / 17 | entity 0x0078ad | 3 of 4 |
| Predator Tank | 0 | 3 | 0 | 232 3 19 0 | 16 0 120 173 | 5 | 4 | 227 0 0 | 0 / no | 1000 / 19 | entity 0x0078ad | 3 of 4 |
| Landraider Redeemer | 0 | 3 | 0 | 232 3 20 0 | 16 0 120 173 | 5 | 4 | 226 0 0 | 0 / no | 1000 / 20 | entity 0x0078ad | 3 of 4 |

#### Orks
| Name | size (hi byte) | opcode | flags | context | receiver | tag | length | payload | sender / process (decoded) | player id / counter (decoded) | receiver (decoded) | payload bytes (decoded) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Sluggas | 0 | 3 | 1 | 233 3 0 0 | 16 0 120 185 | 5 | 4 | 245 0 0 | 1 / no | 1001 / 0 | entity 0x0078b9 | 3 of 4 |
| Shoota Boys | 0 | 3 | 1 | 233 3 1 0 | 16 0 120 185 | 5 | 4 | 244 0 0 | 1 / no | 1001 / 1 | entity 0x0078b9 | 3 of 4 |
| Lootas | 0 | 3 | 1 | 233 3 2 0 | 16 0 120 185 | 5 | 4 | 241 0 0 | 1 / no | 1001 / 2 | entity 0x0078b9 | 3 of 4 |
| Stormboyz | 0 | 3 | 1 | 233 3 3 0 | 16 0 120 185 | 5 | 4 | 248 0 0 | 1 / no | 1001 / 3 | entity 0x0078b9 | 3 of 4 |
| Painboy | 0 | 3 | 1 | 233 3 4 0 | 16 0 120 185 | 5 | 4 | 243 0 0 | 1 / no | 1001 / 4 | entity 0x0078b9 | 3 of 4 |
| Stikkbommaz | 0 | 3 | 1 | 233 3 11 0 | 16 0 120 185 | 5 | 4 | 247 0 0 | 1 / no | 1001 / 11 | entity 0x0078b9 | 3 of 4 |
| Tankbustas | 0 | 3 | 1 | 233 3 12 0 | 16 0 120 185 | 5 | 4 | 249 0 0 | 1 / no | 1001 / 12 | entity 0x0078b9 | 3 of 4 |
| Deff Dread | 0 | 3 | 1 | 233 3 13 0 | 16 0 120 185 | 5 | 4 | 252 0 0 | 1 / no | 1001 / 13 | entity 0x0078b9 | 3 of 4 |
| Wartrukk | 0 | 3 | 1 | 233 3 14 0 | 16 0 120 185 | 5 | 4 | 254 0 0 | 1 / no | 1001 / 14 | entity 0x0078b9 | 3 of 4 |
| Weirdboy | 0 | 3 | 1 | 233 3 19 0 | 16 0 120 185 | 5 | 4 | 250 0 0 | 1 / no | 1001 / 19 | entity 0x0078b9 | 3 of 4 |
| Nob Squad | 0 | 3 | 1 | 233 3 22 0 | 16 0 120 185 | 5 | 4 | 242 0 0 | 1 / no | 1001 / 22 | entity 0x0078b9 | 3 of 4 |
| Kommando Squad | 0 | 3 | 1 | 233 3 23 0 | 16 0 120 185 | 5 | 4 | 240 0 0 | 1 / no | 1001 / 23 | entity 0x0078b9 | 3 of 4 |
| Looted Tank | 0 | 3 | 1 | 233 3 24 0 | 16 0 120 185 | 5 | 4 | 253 0 0 | 1 / no | 1001 / 24 | entity 0x0078b9 | 3 of 4 |
| Battlewaggon | 0 | 3 | 1 | 233 3 28 0 | 16 0 120 185 | 5 | 4 | 251 0 0 | 1 / no | 1001 / 28 | entity 0x0078b9 | 3 of 4 |
| Flash Gitz | 0 | 3 | 1 | 233 3 29 0 | 16 0 120 185 | 5 | 4 | 239 0 0 | 1 / no | 1001 / 29 | entity 0x0078b9 | 3 of 4 |

#### Chaos Space Marines
| Name | size (hi byte) | opcode | flags | context | receiver | tag | length | payload | sender / process (decoded) | player id / counter (decoded) | receiver (decoded) | payload bytes (decoded) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Heretics | 0 | 3 | 1 | 233 3 0 0 | 16 0 120 183 | 5 | 4 | 110 0 0 | 1 / no | 1001 / 0 | entity 0x0078b7 | 3 of 4 |
| Chaos Space Marines | 0 | 3 | 1 | 233 3 1 0 | 16 0 120 183 | 5 | 4 | 121 0 0 | 1 / no | 1001 / 1 | entity 0x0078b7 | 3 of 4 |
| Havocs | 0 | 3 | 1 | 233 3 2 0 | 16 0 120 183 | 5 | 4 | 113 0 0 | 1 / no | 1001 / 2 | entity 0x0078b7 | 3 of 4 |
| Noise Marines | 0 | 3 | 1 | 233 3 3 0 | 16 0 120 183 | 5 | 4 | 115 0 0 | 1 / no | 1001 / 3 | entity 0x0078b7 | 3 of 4 |
| Raptors | 0 | 3 | 1 | 233 3 4 0 | 16 0 120 183 | 5 | 4 | 120 0 0 | 1 / no | 1001 / 4 | entity 0x0078b7 | 3 of 4 |
| Bloodletters | 0 | 3 | 1 | 233 3 11 0 | 16 0 120 183 | 5 | 4 | 103 0 0 | 1 / no | 1001 / 11 | entity 0x0078b7 | 3 of 4 |
| Chaos Dreadnought | 0 | 3 | 1 | 233 3 12 0 | 16 0 120 183 | 5 | 4 | 123 0 0 | 1 / no | 1001 / 12 | entity 0x0078b7 | 3 of 4 |
| Bloodcrusher | 0 | 3 | 1 | 233 3 13 0 | 16 0 120 183 | 5 | 4 | 114 0 0 | 1 / no | 1001 / 13 | entity 0x0078b7 | 3 of 4 |
| Plague Marines | 0 | 3 | 1 | 233 3 14 0 | 16 0 120 183 | 5 | 4 | 118 0 0 | 1 / no | 1001 / 14 | entity 0x0078b7 | 3 of 4 |
| Chaos Predator | 0 | 3 | 1 | 233 3 20 0 | 16 0 120 183 | 5 | 4 | 125 0 0 | 1 / no | 1001 / 20 | entity 0x0078b7 | 3 of 4 |
| Great Unclean One | 0 | 3 | 1 | 233 3 21 0 | 16 0 120 183 | 5 | 4 | 112 0 0 | 1 / no | 1001 / 21 | entity 0x0078b7 | 3 of 4 |
| Landraider Phobos | 0 | 3 | 1 | 233 3 22 0 | 16 0 120 183 | 5 | 4 | 124 0 0 | 1 / no | 1001 / 22 | entity 0x0078b7 | 3 of 4 |

#### Imperial Guard
| Name | size (hi byte) | opcode | flags | context | receiver | tag | length | payload | sender / process (decoded) | player id / counter (decoded) | receiver (decoded) | payload bytes (decoded) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Guardsmen | 0 | 3 | 0 | 232 3 0 0 | 16 0 120 173 | 5 | 4 | 192 0 0 | 0 / no | 1000 / 0 | entity 0x0078ad | 3 of 4 |
| Sentinel | 0 | 3 | 0 | 232 3 1 0 | 16 0 120 173 | 5 | 4 | 204 0 0 | 0 / no | 1000 / 1 | entity 0x0078ad | 3 of 4 |
| Heavy Weapons Squad | 0 | 3 | 0 | 232 3 2 0 | 16 0 120 173 | 5 | 4 | 193 0 0 | 0 / no | 1000 / 2 | entity 0x0078ad | 3 of 4 |
| Catachan Devils | 0 | 3 | 0 | 232 3 3 0 | 16 0 120 173 | 5 | 4 | 191 0 0 | 0 / no | 1000 / 3 | entity 0x0078ad | 3 of 4 |
| Spotters | 0 | 3 | 0 | 232 3 4 0 | 16 0 120 173 | 5 | 4 | 197 0 0 | 0 / no | 1000 / 4 | entity 0x0078ad | 3 of 4 |
| Stormtroopers | 0 | 3 | 0 | 232 3 11 0 | 16 0 120 173 | 5 | 4 | 198 0 0 | 0 / no | 1000 / 11 | entity 0x0078ad | 3 of 4 |
| Chimera | 0 | 3 | 0 | 232 3 12 0 | 16 0 120 173 | 5 | 4 | 201 0 0 | 0 / no | 1000 / 12 | entity 0x0078ad | 3 of 4 |
| Ogryn Squad | 0 | 3 | 0 | 232 3 13 0 | 16 0 120 173 | 5 | 4 | 203 0 0 | 0 / no | 1000 / 13 | entity 0x0078ad | 3 of 4 |
| Manticore | 0 | 3 | 0 | 232 3 14 0 | 16 0 120 173 | 5 | 4 | 196 0 0 | 0 / no | 1000 / 14 | entity 0x0078ad | 3 of 4 |
| Leman Russ | 0 | 3 | 0 | 232 3 20 0 | 16 0 120 173 | 5 | 4 | 202 0 0 | 0 / no | 1000 / 20 | entity 0x0078ad | 3 of 4 |
| Baneblade | 0 | 3 | 0 | 232 3 21 0 | 16 0 120 173 | 5 | 4 | 199 0 0 | 0 / no | 1000 / 21 | entity 0x0078ad | 3 of 4 |
| Kasrkin Squad | 0 | 3 | 0 | 232 3 22 0 | 16 0 120 173 | 5 | 4 | 194 0 0 | 0 / no | 1000 / 22 | entity 0x0078ad | 3 of 4 |

#### Tyranids
| Name | size (hi byte) | opcode | flags | context | receiver | tag | length | payload | sender / process (decoded) | player id / counter (decoded) | receiver (decoded) | payload bytes (decoded) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Hormagaunts | 0 | 3 | 0 | 232 3 0 0 | 16 0 120 173 | 5 | 4 | 5 1 0 | 0 / no | 1000 / 0 | entity 0x0078ad | 3 of 4 |
| Termagants | 0 | 3 | 0 | 232 3 1 0 | 16 0 120 173 | 5 | 4 | 11 1 0 | 0 / no | 1000 / 1 | entity 0x0078ad | 3 of 4 |
| Warrior Brood | 0 | 3 | 0 | 232 3 2 0 | 16 0 120 173 | 5 | 4 | 13 1 0 | 0 / no | 1000 / 2 | entity 0x0078ad | 3 of 4 |
| Ravener Brood | 0 | 3 | 0 | 232 3 3 0 | 16 0 120 173 | 5 | 4 | 7 1 0 | 0 / no | 1000 / 3 | entity 0x0078ad | 3 of 4 |
| Spore Mines | 0 | 3 | 0 | 232 3 4 0 | 16 0 120 173 | 5 | 4 | 9 1 0 | 0 / no | 1000 / 4 | entity 0x0078ad | 3 of 4 |
| Venom Brood | 0 | 3 | 0 | 232 3 11 0 | 16 0 120 173 | 5 | 4 | 14 1 0 | 0 / no | 1000 / 11 | entity 0x0078ad | 3 of 4 |
| Tyrant Guard | 0 | 3 | 0 | 232 3 12 0 | 16 0 120 173 | 5 | 4 | 15 1 0 | 0 / no | 1000 / 12 | entity 0x0078ad | 3 of 4 |
| Genestealer Brood | 0 | 3 | 0 | 232 3 13 0 | 16 0 120 173 | 5 | 4 | 12 1 0 | 0 / no | 1000 / 13 | entity 0x0078ad | 3 of 4 |
| Zoanthrope | 0 | 3 | 0 | 232 3 14 0 | 16 0 120 173 | 5 | 4 | 4 1 0 | 0 / no | 1000 / 14 | entity 0x0078ad | 3 of 4 |
| Carnifex | 0 | 3 | 0 | 232 3 20 0 | 16 0 120 173 | 5 | 4 | 6 1 0 | 0 / no | 1000 / 20 | entity 0x0078ad | 3 of 4 |
| Lictor | 0 | 3 | 0 | 232 3 21 0 | 16 0 120 173 | 5 | 4 | 3 1 0 | 0 / no | 1000 / 21 | entity 0x0078ad | 3 of 4 |
| Swarmlord | 0 | 3 | 0 | 232 3 22 0 | 16 0 120 173 | 5 | 4 | 10 1 0 | 0 / no | 1000 / 22 | entity 0x0078ad | 3 of 4 |
| Neurothrope | 0 | 3 | 0 | 232 3 26 0 | 16 0 120 173 | 5 | 4 | 16 1 0 | 0 / no | 1000 / 26 | entity 0x0078ad | 3 of 4 |

#### Ordo Malleus
| Name | size (hi byte) | opcode | flags | context | receiver | tag | length | payload | sender / process (decoded) | player id / counter (decoded) | receiver (decoded) | payload bytes (decoded) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Inquisitorial Storm Troopers | 0 | 3 | 1 | 233 3 0 0 | 16 0 120 182 | 5 | 4 | 177 0 0 | 1 / no | 1001 / 0 | entity 0x0078b6 | 3 of 4 |
| Strike Squad | 0 | 3 | 1 | 233 3 1 0 | 16 0 120 182 | 5 | 4 | 178 0 0 | 1 / no | 1001 / 1 | entity 0x0078b6 | 3 of 4 |
| Inquistorial Operatives | 0 | 3 | 1 | 233 3 2 0 | 16 0 120 182 | 5 | 4 | 173 0 0 | 1 / no | 1001 / 2 | entity 0x0078b6 | 3 of 4 |
| Purgation Squad | 0 | 3 | 1 | 233 3 3 0 | 16 0 120 182 | 5 | 4 | 175 0 0 | 1 / no | 1001 / 3 | entity 0x0078b6 | 3 of 4 |
| Interceptor Squad | 0 | 3 | 1 | 233 3 4 0 | 16 0 120 182 | 5 | 4 | 169 0 0 | 1 / no | 1001 / 4 | entity 0x0078b6 | 3 of 4 |
| Purifiers | 0 | 3 | 1 | 233 3 11 0 | 16 0 120 182 | 5 | 4 | 176 0 0 | 1 / no | 1001 / 11 | entity 0x0078b6 | 3 of 4 |
| Terminator Librarian | 0 | 3 | 1 | 233 3 12 0 | 16 0 120 182 | 5 | 4 | 170 0 0 | 1 / no | 1001 / 12 | entity 0x0078b6 | 3 of 4 |
| Grey Knight Dreadnought | 0 | 3 | 1 | 233 3 13 0 | 16 0 120 182 | 5 | 4 | 181 0 0 | 1 / no | 1001 / 13 | entity 0x0078b6 | 3 of 4 |
| Vindicare Assassin | 0 | 3 | 1 | 233 3 14 0 | 16 0 120 182 | 5 | 4 | 180 0 0 | 1 / no | 1001 / 14 | entity 0x0078b6 | 3 of 4 |
| Rhino | 0 | 3 | 1 | 233 3 19 0 | 16 0 120 182 | 5 | 4 | 183 0 0 | 1 / no | 1001 / 19 | entity 0x0078b6 | 3 of 4 |
| Grey Knight Terminators | 0 | 3 | 1 | 233 3 22 0 | 16 0 120 182 | 5 | 4 | 179 0 0 | 1 / no | 1001 / 22 | entity 0x0078b6 | 3 of 4 |
| Landraider Crusader | 0 | 3 | 1 | 233 3 23 0 | 16 0 120 182 | 5 | 4 | 182 0 0 | 1 / no | 1001 / 23 | entity 0x0078b6 | 3 of 4 |
| Vortimer Razorback | 0 | 3 | 1 | 233 3 24 0 | 16 0 120 182 | 5 | 4 | 184 0 0 | 1 / no | 1001 / 24 | entity 0x0078b6 | 3 of 4 |


### Wargear Purchases
#### Eldar
##### Farseer
| Name | size (hi byte) | opcode | flags | context | receiver | tag | length | payload | sender / process (decoded) | player id / counter (decoded) | receiver (decoded) | payload bytes (decoded) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Doombringer | 0 | 50 | 1 | 233 3 2 0 | 32 0 195 86 | 5 | 4 | 192 0 0 | 1 / no | 1001 / 2 | squad 0x00c356 | 3 of 4 |
| Fortune Armor | 0 | 50 | 1 | 233 3 3 0 | 32 0 195 86 | 5 | 4 | 184 0 0 | 1 / no | 1001 / 3 | squad 0x00c356 | 3 of 4 |
| Spirit Stones | 0 | 50 | 1 | 233 3 4 0 | 32 0 195 86 | 5 | 4 | 175 0 0 | 1 / no | 1001 / 4 | squad 0x00c356 | 3 of 4 |
| Singing Spear | 0 | 50 | 1 | 233 3 5 0 | 32 0 195 86 | 5 | 4 | 193 0 0 | 1 / no | 1001 / 5 | squad 0x00c356 | 3 of 4 |
| Rune Armor | 0 | 50 | 1 | 233 3 6 0 | 32 0 195 86 | 5 | 4 | 185 0 0 | 1 / no | 1001 / 6 | squad 0x00c356 | 3 of 4 |
| Ghosthelm | 0 | 50 | 1 | 233 3 7 0 | 32 0 195 86 | 5 | 4 | 173 0 0 | 1 / no | 1001 / 7 | squad 0x00c356 | 3 of 4 |
| Gravity Blade | 0 | 50 | 1 | 233 3 10 0 | 32 0 195 86 | 5 | 4 | 194 0 0 | 1 / no | 1001 / 10 | squad 0x00c356 | 3 of 4 |
| Asuryan Armor | 0 | 50 | 1 | 233 3 11 0 | 32 0 195 86 | 5 | 4 | 183 0 0 | 1 / no | 1001 / 11 | squad 0x00c356 | 3 of 4 |
| Runes of Reaping | 0 | 50 | 1 | 233 3 12 0 | 32 0 195 86 | 5 | 4 | 174 0 0 | 1 / no | 1001 / 12 | squad 0x00c356 | 3 of 4 |

##### Warpspider Exarch
| Name | size (hi byte) | opcode | flags | context | receiver | tag | length | payload | sender / process (decoded) | player id / counter (decoded) | receiver (decoded) | payload bytes (decoded) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Heavy Gauge Death Spinner | 0 | 50 | 1 | 233 3 2 0 | 32 0 195 86 | 5 | 4 | 195 0 0 | 1 / no | 1001 / 2 | squad 0x00c356 | 3 of 4 |
| Improved Warp Generator | 0 | 50 | 1 | 233 3 4 0 | 32 0 195 86 | 5 | 4 | 190 0 0 | 1 / no | 1001 / 4 | squad 0x00c356 | 3 of 4 |
| Improved Targeters | 0 | 50 | 1 | 233 3 6 0 | 32 0 195 86 | 5 | 4 | 181 0 0 | 1 / no | 1001 / 6 | squad 0x00c356 | 3 of 4 |
| Entangling Web | 0 | 50 | 1 | 233 3 8 0 | 32 0 195 86 | 5 | 4 | 199 0 0 | 1 / no | 1001 / 8 | squad 0x00c356 | 3 of 4 |
| Enhanced Warp Jump Generator | 0 | 50 | 1 | 233 3 10 0 | 32 0 195 86 | 5 | 4 | 189 0 0 | 1 / no | 1001 / 10 | squad 0x00c356 | 3 of 4 |
| Shimmer Orb | 0 | 50 | 1 | 233 3 12 0 | 32 0 195 86 | 5 | 4 | 182 0 0 | 1 / no | 1001 / 12 | squad 0x00c356 | 3 of 4 |
| Powerblades | 0 | 50 | 1 | 233 3 14 0 | 32 0 195 86 | 5 | 4 | 200 0 0 | 1 / no | 1001 / 14 | squad 0x00c356 | 3 of 4 |
| Phase Armor | 0 | 50 | 1 | 233 3 16 0 | 32 0 195 86 | 5 | 4 | 191 0 0 | 1 / no | 1001 / 16 | squad 0x00c356 | 3 of 4 |
| Anti-Grav Grenade | 0 | 50 | 1 | 233 3 18 0 | 32 0 195 86 | 5 | 4 | 180 0 0 | 1 / no | 1001 / 18 | squad 0x00c356 | 3 of 4 |

##### Warlock
| Name | size (hi byte) | opcode | flags | context | receiver | tag | length | payload | sender / process (decoded) | player id / counter (decoded) | receiver (decoded) | payload bytes (decoded) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Immolator | 0 | 50 | 0 | 232 3 2 0 | 32 0 195 84 | 5 | 4 | 198 0 0 | 0 / no | 1000 / 2 | squad 0x00c354 | 3 of 4 |
| Champion's Robe | 0 | 50 | 0 | 232 3 3 0 | 32 0 195 84 | 5 | 4 | 186 0 0 | 0 / no | 1000 / 3 | squad 0x00c354 | 3 of 4 |
| Channeling Runes | 0 | 50 | 0 | 232 3 4 0 | 32 0 195 84 | 5 | 4 | 176 0 0 | 0 / no | 1000 / 4 | squad 0x00c354 | 3 of 4 |
| Merciless Witchblade | 0 | 50 | 0 | 232 3 5 0 | 32 0 195 84 | 5 | 4 | 197 0 0 | 0 / no | 1000 / 5 | squad 0x00c354 | 3 of 4 |
| Cloak of Shadows | 0 | 50 | 0 | 232 3 6 0 | 32 0 195 84 | 5 | 4 | 187 0 0 | 0 / no | 1000 / 6 | squad 0x00c354 | 3 of 4 |
| Warp Throw | 0 | 50 | 0 | 232 3 12 0 | 32 0 195 84 | 5 | 4 | 178 0 0 | 0 / no | 1000 / 12 | squad 0x00c354 | 3 of 4 |
| Witchblade of Kurnous | 0 | 50 | 0 | 232 3 13 0 | 32 0 195 84 | 5 | 4 | 196 0 0 | 0 / no | 1000 / 13 | squad 0x00c354 | 3 of 4 |
| Providence | 0 | 50 | 0 | 232 3 14 0 | 32 0 195 84 | 5 | 4 | 188 0 0 | 0 / no | 1000 / 14 | squad 0x00c354 | 3 of 4 |
| Heart of Darkness | 0 | 50 | 0 | 232 3 15 0 | 32 0 195 84 | 5 | 4 | 179 0 0 | 0 / no | 1000 / 15 | squad 0x00c354 | 3 of 4 |
| Falochu's Wing | 0 | 50 | 0 | 232 3 16 0 | 32 0 195 84 | 5 | 4 | 177 0 0 | 0 / no | 1000 / 16 | squad 0x00c354 | 3 of 4 |

### Building Purchases

#### Nodes + Generators
##### Nodes
| # | size (hi byte) | opcode | flags | context | receiver | tag | length | payload | sender / process (decoded) | player id / counter (decoded) | receiver (decoded) | payload bytes (decoded) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 0 | 78 | 0 | 232 3 7 0 | 0 0 3 232 | 15 | 35 | 111 2 0 0 38 200 225 66 0 0 72 66 37 216 220 66 38 200 227 66 0 0 72 66 37 216 220 66 0 25 41 0 0 0 | 0 / no | 1000 / 7 | player 1000 | 34 of 35 |
| 2 | 0 | 78 | 3 | 235 3 27 0 | 0 0 3 235 | 15 | 35 | 111 2 0 0 0 0 198 194 0 0 72 66 0 0 210 66 0 0 196 194 0 0 72 66 0 0 210 66 0 15 41 0 0 0 | 3 / no | 1003 / 27 | player 1003 | 34 of 35 |
| 3 | 0 | 78 | 2 | 234 3 22 0 | 0 0 3 234 | 15 | 35 | 111 2 0 0 0 0 158 66 0 0 72 66 0 0 148 194 0 0 160 66 0 0 72 66 0 0 148 194 0 146 43 0 0 0 | 2 / no | 1002 / 22 | player 1002 | 34 of 35 |
| 4 | 0 | 78 | 5 | 237 3 26 0 | 0 0 3 237 | 15 | 35 | 147 1 0 0 0 0 178 194 0 0 72 66 0 0 138 194 0 0 176 194 0 0 72 66 0 0 138 194 0 80 43 0 0 0 | 5 / no | 1005 / 26 | player 1005 | 34 of 35 |
| 5 | 0 | 78 | 0 | 232 3 40 0 | 0 0 3 232 | 15 | 35 | 111 2 0 0 0 0 0 0 0 0 72 66 0 0 60 67 0 0 128 63 0 0 72 66 0 0 60 67 0 211 52 0 0 0 | 0 / no | 1000 / 40 | player 1000 | 34 of 35 |
| 6 | 0 | 78 | 4 | 236 3 216 0 | 0 0 3 236 | 15 | 35 | 36 2 0 0 0 0 128 63 0 0 72 66 0 0 128 191 0 0 0 64 0 0 72 66 0 0 128 191 0 199 52 0 0 0 | 4 / no | 1004 / 216 | player 1004 | 34 of 35 |

##### Generators
| # | size (hi byte) | opcode | flags | context | receiver | tag | length | payload | sender / process (decoded) | player id / counter (decoded) | receiver (decoded) | payload bytes (decoded) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 0 | 78 | 128 | 232 3 41 0 | 0 0 3 232 | 15 | 35 | 102 2 0 0 0 0 225 66 0 0 72 66 0 0 205 66 0 0 227 66 0 0 72 66 0 0 205 66 0 13 55 0 0 0 | 0 / yes | 1000 / 41 | player 1000 | 34 of 35 |
| 2 | 0 | 78 | 128 | 232 3 42 0 | 0 0 3 232 | 15 | 35 | 102 2 0 0 0 0 225 66 0 0 72 66 0 0 205 66 0 0 227 66 0 0 72 66 0 0 205 66 0 13 55 0 0 0 | 0 / yes | 1000 / 42 | player 1000 | 34 of 35 |
| 3 | 0 | 78 | 132 | 236 3 0 1 | 0 0 3 236 | 15 | 35 | 31 2 0 0 0 0 208 192 0 0 72 66 0 0 0 191 0 0 176 192 0 0 72 66 0 0 0 191 0 66 61 0 0 0 | 4 / yes | 1004 / 256 | player 1004 | 34 of 35 |

#### Eldar
| Purchase | size (hi byte) | opcode | flags | context | receiver | tag | length | payload | sender / process (decoded) | player id / counter (decoded) | receiver (decoded) | payload bytes (decoded) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Tier 2 | 0 | 15 | 1 | 233 3 0 0 | 16 0 120 183 | 5 | 4 | 124 0 0 | 1 / no | 1001 / 0 | entity 0x0078b7 | 3 of 4 |
| Tier 3 | 0 | 15 | 1 | 233 3 1 0 | 16 0 120 183 | 5 | 4 | 125 0 0 | 1 / no | 1001 / 1 | entity 0x0078b7 | 3 of 4 |


#### Ordo Malleus

| Purchase | size (hi byte) | opcode | flags | context | receiver | tag | length | payload | sender / process (decoded) | player id / counter (decoded) | receiver (decoded) | payload bytes (decoded) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Tier 2 | 0 | 15 | 0 | 232 3 1 0 | 16 0 120 173 | 5 | 4 | 190 1 0 | 0 / no | 1000 / 1 | entity 0x0078ad | 3 of 4 |


#### Chaos Space Marines

| Purchase | size (hi byte) | opcode | flags | context | receiver | tag | length | payload | sender / process (decoded) | player id / counter (decoded) | receiver (decoded) | payload bytes (decoded) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Tier 2 | 0 | 15 | 1 | 233 3 0 0 | 16 0 120 185 | 5 | 4 | 40 0 0 | 1 / no | 1001 / 0 | entity 0x0078b9 | 3 of 4 |
| Tier 3 | 0 | 15 | 1 | 233 3 1 0 | 16 0 120 185 | 5 | 4 | 41 0 0 | 1 / no | 1001 / 1 | entity 0x0078b9 | 3 of 4 |


#### Imperial Guard

| Purchase | size (hi byte) | opcode | flags | context | receiver | tag | length | payload | sender / process (decoded) | player id / counter (decoded) | receiver (decoded) | payload bytes (decoded) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Tier 2 | 0 | 15 | 1 | 233 3 0 0 | 16 0 120 183 | 5 | 4 | 52 1 0 | 1 / no | 1001 / 0 | entity 0x0078b7 | 3 of 4 |
| Tier 3 | 0 | 15 | 1 | 233 3 1 0 | 16 0 120 183 | 5 | 4 | 54 1 0 | 1 / no | 1001 / 1 | entity 0x0078b7 | 3 of 4 |


#### Orks

| Purchase | size (hi byte) | opcode | flags | context | receiver | tag | length | payload | sender / process (decoded) | player id / counter (decoded) | receiver (decoded) | payload bytes (decoded) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Tier 2 | 0 | 15 | 0 | 232 3 0 0 | 16 0 120 173 | 5 | 4 | 20 2 0 | 0 / no | 1000 / 0 | entity 0x0078ad | 3 of 4 |
| Tier 3 | 0 | 15 | 0 | 232 3 1 0 | 16 0 120 173 | 5 | 4 | 21 2 0 | 0 / no | 1000 / 1 | entity 0x0078ad | 3 of 4 |


#### Space Marines

| Purchase | size (hi byte) | opcode | flags | context | receiver | tag | length | payload | sender / process (decoded) | player id / counter (decoded) | receiver (decoded) | payload bytes (decoded) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Tier 2 | 0 | 15 | 0 | 232 3 0 0 | 16 0 120 173 | 5 | 4 | 190 1 0 | 0 / no | 1000 / 0 | entity 0x0078ad | 3 of 4 |
| Tier 3 | 0 | 15 | 0 | 232 3 1 0 | 16 0 120 173 | 5 | 4 | 191 1 0 | 0 / no | 1000 / 1 | entity 0x0078ad | 3 of 4 |


#### Tyranids

| Purchase | size (hi byte) | opcode | flags | context | receiver | tag | length | payload | sender / process (decoded) | player id / counter (decoded) | receiver (decoded) | payload bytes (decoded) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Tier 2 | 0 | 15 | 0 | 232 3 0 0 | 16 0 120 173 | 5 | 4 | 82 2 0 | 0 / no | 1000 / 0 | entity 0x0078ad | 3 of 4 |
| Tier 3 | 0 | 15 | 0 | 232 3 1 0 | 16 0 120 173 | 5 | 4 | 83 2 0 | 0 / no | 1000 / 1 | entity 0x0078ad | 3 of 4 |
