# Replay action format

The action tables that previously started this file described a fixed 19-byte
record. That description was wrong: replay commands have an inclusive
little-endian `u16` length and a variable body. The old named examples remain
below as historical observations; their labels are hypotheses unless the audit
explicitly marks the underlying fact as proven. See
[`docs/action-table-audit.md`](docs/action-table-audit.md) for the row-by-row
disposition and static evidence,
[`docs/replay-format.md`](docs/replay-format.md) for the container format and
the blueprint/purchase/runtime ID namespaces, and `tools/replay_inspect.py` to
validate replay boundaries before assigning gameplay meanings.

## Current wire layout

Every command is framed inside a bundle command stream as:

```text
u16 command_size                 # little endian, includes these 2 bytes
u8  opcode                       # body offset 0
u8  sender_and_process_flags     # body offset 1
u8[4] reserved_command_context   # body offsets 2..5
... receiver field ...
u8  custom_type                  # 0xff means no custom payload
... custom length and payload ...
```

The sender slot is `flags & 0x7f`; the high bit is the serialized command
process flag. The four context bytes are copied onto the wire verbatim. Native
`CommandIssueProxy` construction fills the first `u16` through a virtual
identity getter and the second through a per-proxy 16-bit counter before
calling `WorldCommand::SetReserved`; the getter's exact semantic name remains
unresolved. The six-file audit corpus has
`u16_le(context[0..2]) == 1000 + sender` for all 30,373 commands. The next
little-endian `u16` usually increments from zero per sender, but `recs/1.rec`
has one stream reset from 1037 to zero; no counter limit or wrap rule is
proven. A
single receiver is a big-endian packed word
`(type << 28) | instance_id`. A receiver list has a tagged big-endian count
(`0x40..`, `0x8000..`, or `0xc0000000..`) followed by little-endian packed
receiver words. Observed receiver types are player `0`, entity `1`, and squad
`2`. These runtime instance IDs are not blueprint IDs.

Custom lengths below `0x80` occupy one byte; larger lengths use
`((first & 0x7f) << 8) | second`. Type `0xff` means no custom payload. Every
custom tag observed in the corpus now has a recovered layout built from three
primitives — `u32`, 12-byte position, and a packed Target
(`u8 type` + nothing/`u16` squad/`u32` entity/12-byte position, wire sizes
1/3/5/13) — plus a packed builder entity `u32`/squad `u16` pair in the
construction forms. See the custom-tag codec table in the audit. Tags outside
the recovered set stay opaque.

For the analysed build the opcode byte is also named: the executable preserves
the command enum in its SCAR constant registration, in three name-pointer
arrays (`CMD_*` base 0, `SCMD_*` base 41, `PCMD_*` base 78), and in the
literal opcodes passed to the `WorldDoCommand*` issuers — three independent
sources that agree everywhere they overlap. The type-5/length-4 `u32` values
resolve per opcode through `PropertyBagGroupManager::GetGroup(type, id)`:
squad-blueprint (SBP) indices for opcode 3, `Upgrade\` indices for 15 and 50,
entity-blueprint (EBP) indices for 49, and `Ability\` indices for the tag-25
ability forms of 13/53/85; opcode 78/89/90's 35-byte form leads with the EBP
index of the structure. Cancellation values (opcodes 5 and 51) are 1-based
per-producer enqueue ordinals — runtime tokens, not blueprints. Blueprint
indices are per-type catalogue positions of the build that recorded the
replay: RBF builds assign them by case-insensitive (`_stricmp`) path order,
RB2 builds by entry order, so raw values are incomparable across versions
unless keyed by the header checksum. See the
[audit](docs/action-table-audit.md) for the enum sources, receive-side
dispatch, codec layouts, and all call-site addresses.

The parser's complete structural view is `ReplayInfo.commands`. The older
`ReplayInfo.actions` view is filtered for compatibility and omits some opcode
values. For a JSON dump, run `cargo run -p parser -- path/to/replay.rec` and
inspect `.commands`.

The old fixed rows are not a cross-version ID catalogue. A receiver instance,
a purchase or queue value, and an attribute blueprint/group index belong to
different namespaces. A replay records numeric references plus a data checksum;
it does not embed the complete mod attribute archive. Matching a number to a
unit name across game or mod versions therefore requires the matching data and
reconstruction of that session's runtime group creation order.

## Resolving blueprint names (`--module`)

Pass the install's module file to turn blueprint indices into names:

```text
cargo run -q -p parser -- temp.rec --module "<install>/Elite.module"
```

The parser reads the module's `[attrib:common]` SGA archives and builds four
catalogues: `ebp` (`simulation\attrib\ebps\`), `sbp` (`sbps\`), `upgrade` and
`ability`. The rule is: every `.rbf` path under that directory across all the
archives, lowercased, deduplicated, byte-sorted; index *i* is the *i*-th path.
Two parts of this rule are not discriminated by the data: including
`Elite_Campaign_Attrib.sga` is a hypothesis, and whether directories sorting after
`ebps\` belong to the EBP namespace is open (this affects only the EBP count); see
`docs/replay-format.md`.
For the build with `mod_chksum` 4156674300 (`0xf7c1d0fc`) the counts are ebp 2591,
sbp 715, upgrade 1751, ability 2399.

Each command whose (opcode, custom tag, length) is a verified blueprint form
gets `command.blueprint = {"kind","index","name","path","display_name"}` (`name` is the file
stem, `path` the path relative to the namespace directory; basenames can
collide across directories, so use `path` when in doubt; `display_name` is the
readable name, see below). Resolved forms:

| opcode | form | kind | status |
|---|---|---|---|
| 3 `CMD_BuildSquad` | tag 5, len 4 | `sbp` | verified |
| 15 `CMD_Upgrade`, 50 `SCMD_Upgrade` | tag 5, len 4 | `upgrade` | verified |
| 49 `SCMD_ReinforceUnit` | tag 5, len 4 | `ebp` | verified |
| 78 `PCMD_ConstructStructure`, 89 `PCMD_ConstructFence` | tag 15 / 16, len 35, leading `u32` | `ebp` | verified (89: only 3 commands seen) |
| 53 `SCMD_Ability`, 85 `PCMD_Ability` | tags 25..29, len >= 5, leading `u32` | `ability` | 53 verified for tags 25 and 26; 85 verified for tags 25, 26, 28 and 29 (the resolver accepts 25..=29) |

Global (commander) abilities are opcode 85 and resolve into
`ability\pvp\race_*\globals\`; there is no separate commander namespace.
Deliberately not resolved: opcode 13 (`CMD_Ability`, unverified), 90 and 4/16/63/64
(documented only), 5/51 (queue ordinals), 71 (stance), tag-1 forms of 53/85, and 94/96
(not blueprints).

`ReplayInfo.blueprints` records `module` and the catalogue sizes (`ebp_count`,
`sbp_count`, `upgrade_count`, `ability_count`). With `--module`, names are always
resolved; there is no build check.

Indices are build-specific catalogue positions. Resolving a replay recorded on
another build than the installed one gives wrong names that are still in range,
without any warning (in the 2015 `3v3.rec` the Imperial Guard player "builds" `gk_*`
squads). The replay's `mod_chksum` is part of the output, so callers can compare
builds themselves. `mod_chksum` cannot yet be computed from the archives;
4156674300 is associated with the analysed install by data fit, not by derivation.

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

### Display names

With `--module`, each resolved `blueprint` also gets `display_name`: the blueprint's
`screen_name` (a localisation id stored in its RBF attribute file), looked up in
`DOW2.ucs` plus `<module name>.ucs` (`Elite.ucs`) from `GameAssets\Locale\English\`
and trimmed. Only English is supported and there is no language flag. For `temp.rec`
every one of the 1014 `blueprint` objects (counting `commands` and `actions`) has a
`display_name`, with empty stderr; the 166 distinct blueprints of that replay all resolve.

- **Fallback.** `display_name` is absent (not `null`) when the blueprint has no
  name: no `screen_name` field, id 0, or an id missing from the UCS files. `name` and
  `path` are always there.
- **Warnings (stderr).** `warning: display names unavailable: <error>` once, if the
  name data cannot be loaded (no `fieldnames.flb`, `DOW2.ucs` or locale folder, or a
  needed field name missing); no `display_name` is then written. A blueprint whose RBF
  is missing or malformed prints `warning: no display name for <kind> <path>: <error>`
  once and is left without a name. Neither stops the parse.
- **Names are not unique** ("Repair" belongs to three blueprints), so use `path` as the
  identity. Reinforce (opcode 49) resolves to the *model* entity, and some models carry another
  unit's string (ebp `sm_sternguard_veteran` shows "Tactical Marine"), so the name can differ
  from the squad that was reinforced.
- **Other builds.** On a replay from another build than the installed one, display
  names are as wrong as the indices (see above), but they read more convincingly than a
  file stem. Compare `mod_chksum` before trusting either.

The RBF, field-name and UCS formats, the archive override rule and what is verified or
hypothesis are in [`docs/game-data-formats.md`](docs/game-data-formats.md).

## Historical action observations

# General observations:

The bullets in this historical section record patterns reported from a small sample. They are not confirmed wire-field meanings. In particular, pairs such as `(25, 5)` are observed custom type/length shapes; they do not globally identify an ability category, target kind, or unit ID.

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

# Opcode observations

The original opcode names are retained in the named examples below as
historical hypotheses. The six-file corpus observes 27 opcode bytes; all of
them now have engine names recovered from the analysed build (build-scoped,
not a cross-version contract). See
[`docs/action-table-audit.md`](docs/action-table-audit.md) for every old row,
its observed custom shapes, the enum evidence, and its disposition.

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

# Global Abilities

The tables below are preserved raw/historical examples. Their column headings follow the current wire layout (see "Current wire layout" above). The leading `0` is read as the high byte of the `u16` command size; the samples appear to have been captured one byte late, so they omit the size low byte and the command's final byte, and a row therefore normally shows one payload byte fewer than the declared length (the last column counts payload bytes present against the declared length). Columns marked "(decoded)" are derived from the raw bytes and are not part of the samples. Some rows contain a complete variable command tail and some are incomplete; the names and numeric IDs are historical, build-specific observations and not a cross-version ID map. Use the corrected layout and audit above when interpreting these bytes.


## Space Marines (Techmarine, Apothecary, Force Commander)

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

## Eldar (Warlock, WSE, Farseer)

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

# Unit Abilities
## Space Marines
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

## Eldar
| Name | size (hi byte) | opcode | flags | context | receiver | tag | length | payload | sender / process (decoded) | player id / counter (decoded) | receiver (decoded) | payload bytes (decoded) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Farseer Fleet of Foot | 0 | 53 | 1 | 233 3 8 0 | 32 0 195 86 | 25 | 5 | 173 1 0 0 | 1 / no | 1001 / 8 | squad 0x00c356 | 4 of 5 |
| Farseer Guide | 0 | 53 | 1 | 233 3 9 0 | 32 0 195 86 | 26 | 9 | 182 1 0 0 1 0 4 87 | 1 / no | 1001 / 9 | squad 0x00c356 | 8 of 9 |

# Unit Purchases
## Eldar
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

## Space Marines
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

## Orks
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

## Chaos Space Marines
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

## Imperial Guard
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

## Tyranids
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

## Ordo Malleus
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


# Wargear Purchases
## Eldar
### Farseer
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

### Warpspider Exarch
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

### Warlock
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

# Building Purchases

## Nodes + Generators
### Nodes
| # | size (hi byte) | opcode | flags | context | receiver | tag | length | payload | sender / process (decoded) | player id / counter (decoded) | receiver (decoded) | payload bytes (decoded) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 0 | 78 | 0 | 232 3 7 0 | 0 0 3 232 | 15 | 35 | 111 2 0 0 38 200 225 66 0 0 72 66 37 216 220 66 38 200 227 66 0 0 72 66 37 216 220 66 0 25 41 0 0 0 | 0 / no | 1000 / 7 | player 1000 | 34 of 35 |
| 2 | 0 | 78 | 3 | 235 3 27 0 | 0 0 3 235 | 15 | 35 | 111 2 0 0 0 0 198 194 0 0 72 66 0 0 210 66 0 0 196 194 0 0 72 66 0 0 210 66 0 15 41 0 0 0 | 3 / no | 1003 / 27 | player 1003 | 34 of 35 |
| 3 | 0 | 78 | 2 | 234 3 22 0 | 0 0 3 234 | 15 | 35 | 111 2 0 0 0 0 158 66 0 0 72 66 0 0 148 194 0 0 160 66 0 0 72 66 0 0 148 194 0 146 43 0 0 0 | 2 / no | 1002 / 22 | player 1002 | 34 of 35 |
| 4 | 0 | 78 | 5 | 237 3 26 0 | 0 0 3 237 | 15 | 35 | 147 1 0 0 0 0 178 194 0 0 72 66 0 0 138 194 0 0 176 194 0 0 72 66 0 0 138 194 0 80 43 0 0 0 | 5 / no | 1005 / 26 | player 1005 | 34 of 35 |
| 5 | 0 | 78 | 0 | 232 3 40 0 | 0 0 3 232 | 15 | 35 | 111 2 0 0 0 0 0 0 0 0 72 66 0 0 60 67 0 0 128 63 0 0 72 66 0 0 60 67 0 211 52 0 0 0 | 0 / no | 1000 / 40 | player 1000 | 34 of 35 |
| 6 | 0 | 78 | 4 | 236 3 216 0 | 0 0 3 236 | 15 | 35 | 36 2 0 0 0 0 128 63 0 0 72 66 0 0 128 191 0 0 0 64 0 0 72 66 0 0 128 191 0 199 52 0 0 0 | 4 / no | 1004 / 216 | player 1004 | 34 of 35 |

### Generators
| # | size (hi byte) | opcode | flags | context | receiver | tag | length | payload | sender / process (decoded) | player id / counter (decoded) | receiver (decoded) | payload bytes (decoded) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 0 | 78 | 128 | 232 3 41 0 | 0 0 3 232 | 15 | 35 | 102 2 0 0 0 0 225 66 0 0 72 66 0 0 205 66 0 0 227 66 0 0 72 66 0 0 205 66 0 13 55 0 0 0 | 0 / yes | 1000 / 41 | player 1000 | 34 of 35 |
| 2 | 0 | 78 | 128 | 232 3 42 0 | 0 0 3 232 | 15 | 35 | 102 2 0 0 0 0 225 66 0 0 72 66 0 0 205 66 0 0 227 66 0 0 72 66 0 0 205 66 0 13 55 0 0 0 | 0 / yes | 1000 / 42 | player 1000 | 34 of 35 |
| 3 | 0 | 78 | 132 | 236 3 0 1 | 0 0 3 236 | 15 | 35 | 31 2 0 0 0 0 208 192 0 0 72 66 0 0 0 191 0 0 176 192 0 0 72 66 0 0 0 191 0 66 61 0 0 0 | 4 / yes | 1004 / 256 | player 1004 | 34 of 35 |

## Eldar
| Purchase | size (hi byte) | opcode | flags | context | receiver | tag | length | payload | sender / process (decoded) | player id / counter (decoded) | receiver (decoded) | payload bytes (decoded) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Tier 2 | 0 | 15 | 1 | 233 3 0 0 | 16 0 120 183 | 5 | 4 | 124 0 0 | 1 / no | 1001 / 0 | entity 0x0078b7 | 3 of 4 |
| Tier 3 | 0 | 15 | 1 | 233 3 1 0 | 16 0 120 183 | 5 | 4 | 125 0 0 | 1 / no | 1001 / 1 | entity 0x0078b7 | 3 of 4 |


## Ordo Malleus

| Purchase | size (hi byte) | opcode | flags | context | receiver | tag | length | payload | sender / process (decoded) | player id / counter (decoded) | receiver (decoded) | payload bytes (decoded) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Tier 2 | 0 | 15 | 0 | 232 3 1 0 | 16 0 120 173 | 5 | 4 | 190 1 0 | 0 / no | 1000 / 1 | entity 0x0078ad | 3 of 4 |


## Chaos Space Marines

| Purchase | size (hi byte) | opcode | flags | context | receiver | tag | length | payload | sender / process (decoded) | player id / counter (decoded) | receiver (decoded) | payload bytes (decoded) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Tier 2 | 0 | 15 | 1 | 233 3 0 0 | 16 0 120 185 | 5 | 4 | 40 0 0 | 1 / no | 1001 / 0 | entity 0x0078b9 | 3 of 4 |
| Tier 3 | 0 | 15 | 1 | 233 3 1 0 | 16 0 120 185 | 5 | 4 | 41 0 0 | 1 / no | 1001 / 1 | entity 0x0078b9 | 3 of 4 |


## Imperial Guard

| Purchase | size (hi byte) | opcode | flags | context | receiver | tag | length | payload | sender / process (decoded) | player id / counter (decoded) | receiver (decoded) | payload bytes (decoded) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Tier 2 | 0 | 15 | 1 | 233 3 0 0 | 16 0 120 183 | 5 | 4 | 52 1 0 | 1 / no | 1001 / 0 | entity 0x0078b7 | 3 of 4 |
| Tier 3 | 0 | 15 | 1 | 233 3 1 0 | 16 0 120 183 | 5 | 4 | 54 1 0 | 1 / no | 1001 / 1 | entity 0x0078b7 | 3 of 4 |


## Orks

| Purchase | size (hi byte) | opcode | flags | context | receiver | tag | length | payload | sender / process (decoded) | player id / counter (decoded) | receiver (decoded) | payload bytes (decoded) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Tier 2 | 0 | 15 | 0 | 232 3 0 0 | 16 0 120 173 | 5 | 4 | 20 2 0 | 0 / no | 1000 / 0 | entity 0x0078ad | 3 of 4 |
| Tier 3 | 0 | 15 | 0 | 232 3 1 0 | 16 0 120 173 | 5 | 4 | 21 2 0 | 0 / no | 1000 / 1 | entity 0x0078ad | 3 of 4 |


## Space Marines

| Purchase | size (hi byte) | opcode | flags | context | receiver | tag | length | payload | sender / process (decoded) | player id / counter (decoded) | receiver (decoded) | payload bytes (decoded) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Tier 2 | 0 | 15 | 0 | 232 3 0 0 | 16 0 120 173 | 5 | 4 | 190 1 0 | 0 / no | 1000 / 0 | entity 0x0078ad | 3 of 4 |
| Tier 3 | 0 | 15 | 0 | 232 3 1 0 | 16 0 120 173 | 5 | 4 | 191 1 0 | 0 / no | 1000 / 1 | entity 0x0078ad | 3 of 4 |


## Tyranids

| Purchase | size (hi byte) | opcode | flags | context | receiver | tag | length | payload | sender / process (decoded) | player id / counter (decoded) | receiver (decoded) | payload bytes (decoded) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Tier 2 | 0 | 15 | 0 | 232 3 0 0 | 16 0 120 173 | 5 | 4 | 82 2 0 | 0 / no | 1000 / 0 | entity 0x0078ad | 3 of 4 |
| Tier 3 | 0 | 15 | 0 | 232 3 1 0 | 16 0 120 173 | 5 | 4 | 83 2 0 | 0 / no | 1000 / 1 | entity 0x0078ad | 3 of 4 |
