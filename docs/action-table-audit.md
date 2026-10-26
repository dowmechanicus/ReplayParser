# Action table audit

This note audits every opcode label in the original README against the
recoverable replay writer/reader evidence. The executable and disassembly used
for the static analysis are not part of this repository, so addresses below
identify the analysed build and are not universal offsets for every release.
The evidence establishes the command envelope, the generic codecs, and — for
the analysed build — the complete opcode-to-name enum, recovered from three
independent in-binary sources that agree on every cross-checkable value.

Disposition summary: all 27 original rows have a structurally valid opcode
number, 25 occur in the six-file corpus, and 2 (1 and 98) do not occur there.
Every observed opcode now has its engine name (`CMD_*`/`SCMD_*`/`PCMD_*`, see
the recovered enum below), and every `(custom_type, custom_length)` pair in
the corpus has a verified payload layout. The names are a build fact for the
pinned executables: SCAR-visible names are proven by explicit value
registration; the remainder rest on name-array position ("supported" tier)
and must not be assumed stable across game or mod versions.

## Correct wire model

The old table treated a command as a fixed 19-byte structure. The writer emits
an inclusive little-endian `u16` length followed by a variable command body:

```text
u16 command_size                 # includes these two bytes
u8  opcode                       # body offset 0
u8  sender_and_process_flags     # body offset 1
u8[4] reserved_command_context   # body offsets 2..5, copied verbatim
... receiver field ...
u8  custom_type                  # 0xff means no custom payload
... custom length and payload ...
```

The sender byte uses `sender = flags & 0x7f` and
`process = (flags >> 7) & 1`. Static analysis ties the high bit to the
serialized command process field (`WorldCommand::GetCommandProcessType` and
`WorldCommand::IsQueued`), while the low seven bits follow the engine's player
base convention. The four context bytes are wire-raw, but their command-issue
origin is partly recovered. `CommandIssueProxy` construction at `0xaf318f`
creates the first reserved `u16` through a virtual identity getter and the
second through a per-proxy counter; the issue method copies both words, calls
`WorldCommand::SetReserved`, and increments that counter. The getter's exact
game semantic name and any absolute counter limit remain unresolved.

A single receiver is a big-endian packed word:

```text
(receiver_type << 28) | instance_id
```

The observed receiver types are 0=player, 1=entity, and 2=squad. A receiver
list starts with a tagged big-endian count (`0x40..`, `0x8000..`, or
`0xc0000000..`) and stores each packed receiver word little endian. The type
nibble and the low 28-bit instance ID are runtime receiver fields; neither is
an attribute blueprint ID.

For a custom type other than `0xff`, a length below `0x80` is one byte. A
length with the high bit set uses two bytes:
`((first & 0x7f) << 8) | second`. Type 5 is a four-byte unsigned value
(`FlagData`); its gameplay meaning depends on the opcode (see the blueprint
namespace table below). Every custom tag observed in the corpus now has a
recovered layout (see the custom-tag codec section); a tag outside that set
must still be treated as opaque.

The generic writer/reader evidence is in the analysed DOW2 build at virtual
addresses `0x41e8da..0x41ed80` (command writer/reader), `0x41e61b` and
`0x41e5c8` (receiver helpers), and `0x41ebcc` (decoder path). The command
object offsets recovered from the constructors are `+0x00` opcode, `+0x04`
process value, `+0x0c` sender pointer, `+0x10` receiver storage, `+0x40`
custom payload, `+0x44` payload length, `+0x48` custom type, and `+0x49`
trusted state. The trusted state is not serialized.

The native evidence was taken from `DOW2.exe` SHA-256
`4ce6423d3d97528a784ede2a16bb9b671aac221fdde1fddee5bd060cfd38a50e` and
`SimEngine.dll` SHA-256
`a6d503cefe9085e9ee5ca707c8621e8855243b272fb93d6506e9f628a8c89af4`.
The more precise writer field sites are `0x41e94a` (opcode), `0x41e95b`
(process), `0x41e961`/`0x41e97a` (sender and flag), `0x41e98e` (reserved),
`0x41e9d3` (receivers), `0x41e9d8` (custom type), and `0x41e9f0` (custom
length). These addresses identify one analysed x86 build, not a stable public
ABI.

## Worked byte examples

This 18-byte command is a recorded opcode-3 command:

```text
12 00 03 03 eb 03 00 00 10 00 77 0c 05 04 86 00 00 00
```

The length is `0x12`; the body is the remaining 16 bytes. It decodes as:

| field | value |
| --- | --- |
| opcode | `3` |
| sender/process | sender slot `3`, process `0` |
| reserved context | `eb 03 00 00` (raw) |
| receiver | packed `0x1000770c`; type `1`, instance `30476` |
| custom | type `5`, length `4`, raw value `86 00 00 00` (`134` little endian) |

The old “unit ID” byte would have exposed only `0x0c`, the low byte of the
runtime receiver instance. That is why the same apparent unit ID changes
across sessions and versions.

A strict scan of all **30,373 commands** in the six-file audit corpus found
`u16_le(body[2..4]) == 1000 + (body[1] & 0x7f)` for every command. The next
reserved `u16` generally starts at zero and increases per sender, but
`recs/1.rec` has one sender stream reset from 1037 to zero. The serializer
only copies these four bytes. The native origin explains the observed pair as
identity-getter output plus per-proxy sequence state, while the supplied
evidence proves no 65,536-command limit or wraparound rule.

The relevant imports are `WorldCommand` constructor `0xf880dc` and
`SetReserved` `0xf880e4`. The setter xrefs are `0x41ed42` (reader-side
reconstruction) and `0xaf3138` (command-issue proxy); `0x985cf7` is a
constructor call and must not be used as a setter xref. The RTTI at
`0xaf318f` identifies the proxy as `CommandIssueProxy`, and its issue method
reads the virtual identity result into the first word, reads/increments the
16-bit counter for the second, then stores the four-byte reserved value.

This synthetic 18-byte command exercises a two-receiver list and the no-custom
sentinel:

```text
12 00 44 03 eb 03 00 00 42 34 12 00 10 cd ab 00 20 ff
```

The `0x42` count tag means two receivers. Their little-endian words decode to
`0x10001234` (type 1, instance `0x1234`) and `0x2000abcd` (type 2, instance
`0xabcd`). The final `0xff` is a custom-type sentinel with no length or
payload. The list count, receiver endianness, and sentinel are structural
facts; the opcode's gameplay name is not inferred here.

The worked examples are checked without external dependencies by running
`python3 tools/verify_action_table.py`.

## Recovered opcode enum

The analysed build preserves the command-type enum in three independent
places, which agree on every value that more than one source names:

1. **SCAR/Lua constant registration.** Two code blocks
   (`0xb154a9..0xb15830` and `0x93ba78..0x93bdd6`) register each name string
   with an IEEE-754 double value (`fldl` constant, then `push <name string>`,
   then the registration call). Example: `SCMD_Move = 44` registered at
   `0xb155dc` and again at `0x93bac8`.
2. **Name-pointer arrays in `.data`.** Three contiguous `char*` arrays:
   `CMD_*` at `0x12bbd20` (41 entries, index = opcode, base 0), `SCMD_*` at
   `0x12bbdc8` (36 entries, base 41), `PCMD_*` at `0x12bbe58` (22 entries,
   base 78). Every Lua-registered name sits at exactly its value's slot (the
   one exception is `SCMD_DefaultAction`, registered to scripts as the shared
   sentinel 0 while occupying array slot 41).
3. **Issuer call sites.** SimEngine exports
   `WorldDoCommandEntity/Entities/Squad/Squads/Player` (stdcall; the opcode is
   the first parameter, pushed last). The build has 217 `call [IAT]` sites to
   them; 179 pass a literal `push imm8` opcode, and the literals partition
   exactly as the enum predicts: 1–39 only into the Entity issuers, 43–76 only
   into the Squad issuers, 78–99 only into the Player issuers — matching the
   receiver-type nibbles 1/2/0 observed on the wire. The remaining sites push
   a register: they are the SCAR bindings (`0x937991..0x93b8c8`) and relay
   stubs that forward the script-supplied constant from source 1 into the same
   issuers, which is what ties the SCAR values to the wire byte.

Evidence tiers: a name whose value is Lua-registered is **proven** for this
build. A name that appears only at an array position (for example
`SCMD_SetMoveType` 62, `SCMD_Patrol` 66, `PCMD_WaitObjectDone` 95) is
**supported**: every array position that can be cross-checked matches, and
several such names are independently corroborated by their receive-side
handlers, but no explicit value constant names them. All names are
build-scoped; a mod or other game version may renumber them.

### Full name table (all 100 slots)

The three name-pointer arrays were read directly from `DOW2.exe` (2026-10-26)
with the opcode rule above (`CMD_*` index = opcode, `SCMD_*` base 41,
`PCMD_*` base 78). The parser's `Opcode` enum names 28 of these; the rest
decode as `Opcode::Unknown`. Tier: **supported** (array position) unless a
second source is listed. Opcode 77 has no slot. The `CMD_*` array has a 42nd
entry, the string `last_stand_scar`, at the position of opcode 41; it is not
a command name (41 is `SCMD_DefaultAction`).

| opcode | engine name | in parser enum | corpus | second source |
|---|---|---|---|---|
| 0 | `CMD_DefaultAction` | no | not observed | |
| 1 | `CMD_Stop` | yes | not observed | |
| 2 | `CMD_Destroy` | yes | observed | |
| 3 | `CMD_BuildSquad` | yes | observed | |
| 4 | `CMD_InstantBuildSquad` | no | not observed | cheat/instant permission table `0x12bbec8` |
| 5 | `CMD_CancelProduction` | yes | observed | |
| 6 | `CMD_BuildStructure` | no | not observed | |
| 7 | `CMD_Move` | no | not observed | |
| 8 | `CMD_Face` | no | not observed | |
| 9 | `CMD_Attack` | yes | observed | |
| 10 | `CMD_AttackMove` | no | not observed | |
| 11 | `CMD_RallyPoint` | yes | observed | |
| 12 | `CMD_Capture` | no | not observed | |
| 13 | `CMD_Ability` | yes | observed | |
| 14 | `CMD_Evacuate` | no | not observed | |
| 15 | `CMD_Upgrade` | yes | observed | |
| 16 | `CMD_InstantUpgrade` | no | not observed | permission table `0x12bbec8` |
| 17 | `CMD_ChooseResource` | no | not observed | |
| 18 | `CMD_Load` | no | not observed | |
| 19 | `CMD_InstantLoad` | no | not observed | |
| 20 | `CMD_Unload` | no | not observed | |
| 21 | `CMD_MoveToSlot` | no | not observed | |
| 22 | `CMD_MoveAwayFromSlot` | no | not observed | |
| 23 | `CMD_UnloadSquads` | yes | observed | |
| 24 | `CMD_AttackStop` | no | not observed | |
| 25 | `CMD_AttackForced` | no | not observed | |
| 26 | `CMD_DirectControlAim` | no | not observed | |
| 27 | `CMD_DirectControlShoot` | no | not observed | |
| 28 | `CMD_Halt` | no | not observed | |
| 29 | `CMD_Fidget` | no | not observed | |
| 30 | `CMD_Casualty` | no | not observed | |
| 31 | `CMD_Death` | no | not observed | |
| 32 | `CMD_InstantDeath` | no | not observed | |
| 33 | `CMD_Projectile` | no | not observed | |
| 34 | `CMD_MeleeAttack` | no | not observed | |
| 35 | `CMD_Jump` | no | not observed | |
| 36 | `CMD_Knockback` | no | not observed | |
| 37 | `CMD_SetWeaponStance` | no | not observed | |
| 38 | `CMD_Unconscious` | no | not observed | |
| 39 | `CMD_GrantWargear` | no | not observed | |
| 40 | `CMD_Falling` | no | not observed | |
| 41 | `SCMD_DefaultAction` | no | not observed | |
| 42 | `SCMD_FlushQueue` | no | not observed | |
| 43 | `SCMD_Stop` | yes | observed | |
| 44 | `SCMD_Move` | yes | observed | |
| 45 | `SCMD_Destroy` | no | not observed | |
| 46 | `SCMD_BuildStructure` | no | not observed | |
| 47 | `SCMD_Capture` | yes | observed | |
| 48 | `SCMD_Attack` | yes | observed | |
| 49 | `SCMD_ReinforceUnit` | yes | observed | |
| 50 | `SCMD_Upgrade` | yes | observed | |
| 51 | `SCMD_CancelProduction` | yes | observed | |
| 52 | `SCMD_AttackMove` | yes | observed | |
| 53 | `SCMD_Ability` | yes | observed | |
| 54 | `SCMD_Attach` | no | not observed | |
| 55 | `SCMD_Detach` | no | not observed | |
| 56 | `SCMD_Load` | yes | observed | |
| 57 | `SCMD_InstantLoad` | no | not observed | permission table `0x12bbec8` |
| 58 | `SCMD_UnloadSquads` | yes | not observed | |
| 59 | `SCMD_Unload` | no | not observed | |
| 60 | `SCMD_SlotItemRemove` | no | not observed | |
| 61 | `SCMD_Retreat` | yes | observed | |
| 62 | `SCMD_SetMoveType` | no | not observed | |
| 63 | `SCMD_InstantReinforceUnit` | no | not observed | permission table `0x12bbec8` |
| 64 | `SCMD_InstantUpgrade` | no | not observed | permission table `0x12bbec8` |
| 65 | `SCMD_SetCamouflageStance` | no | not observed | |
| 66 | `SCMD_Patrol` | no | not observed | |
| 67 | `SCMD_Surprise` | no | not observed | |
| 68 | `SCMD_StationaryAttack` | no | not observed | |
| 69 | `SCMD_Face` | no | not observed | |
| 70 | `SCMD_Melee` | yes | observed | |
| 71 | `SCMD_SetWeaponStance` | yes | observed | |
| 72 | `SCMD_Jump` | no | not observed | |
| 73 | `SCMD_HunkerDown` | no | not observed | |
| 74 | `SCMD_Pickup` | no | not observed | |
| 75 | `SCMD_Plan` | no | not observed | |
| 76 | `SCMD_Worship` | no | not observed | |
| 78 | `PCMD_ConstructStructure` | yes | observed | |
| 79 | `PCMD_ChangeAlliance` | no | not observed | ModPlayer switch routes it to the reject label `0x7e79f9` |
| 80 | `PCMD_RequisitionDonation` | no | not observed | permission table `0x12bbec8` |
| 81 | `PCMD_MunitionDonation` | no | not observed | permission table `0x12bbec8` |
| 82 | `PCMD_CheatResources` | no | not observed | permission table; UI issuer `push 0x52` |
| 83 | `PCMD_CheatRevealAll` | no | not observed | permission table; UI issuer `push 0x53` |
| 84 | `PCMD_CheatKillSelf` | no | not observed | permission table; UI issuer `push 0x54`; defeats the sender |
| 85 | `PCMD_Ability` | yes | observed | |
| 86 | `PCMD_CheatBuildTime` | no | not observed | permission table; UI issuer `push 0x56` |
| 87 | `PCMD_Upgrade` | no | not observed | `CommandProxy_Upgrade` issuer |
| 88 | `PCMD_InstantUpgrade` | no | not observed | permission table `0x12bbec8` |
| 89 | `PCMD_ConstructFence` | yes | observed | |
| 90 | `PCMD_ConstructField` | no | not observed | `CommandProxy_ConstructField` issuer |
| 91 | `PCMD_UpgradeRemove` | no | not observed | |
| 92 | `PCMD_SlotItemRemove` | no | not observed | `CommandProxy_SlotItemRemove` issuer |
| 93 | `PCMD_CancelProduction` | no | not observed | `CommandProxy_CancelProduction` issuer |
| 94 | `PCMD_Ping` | yes | observed | |
| 95 | `PCMD_WaitObjectDone` | no | not observed | |
| 96 | `PCMD_ResurrectHero` | yes | observed | |
| 97 | `PCMD_ConcessionSuccessful` | no | observed (`temp.rec`, 5×) | ModPlayer switch case → handler `0x7eac80` |
| 98 | `PCMD_SetPreferredRetreatTarget` | no | not observed | live issuers `0x899da7`, `0xb5f93f` |
| 99 | `PCMD_CheatLastStandSelfInvulnerability` | no | not observed | permission table; SCAR issuer `push 0x63` |

"Corpus" is the six replays in this repository (`recs/*.rec`, `3v3.rec`,
`temp.rec`), counted over the parser's full `commands` list on 2026-10-26.

#### Opcode 97 `PCMD_ConcessionSuccessful` in the corpus

`temp.rec` (3v3, Victory Point Control under the Elite mod, 16160 ticks) ends by concession. Five of the
six players issue opcode 97 at ticks 16154–16155. Each command targets the
sender's own player (receiver kind 0, id `1000 + slot`) and carries custom tag
41, length 4, payload `u32 1`. Slot 4 issues no opcode 97 although it sends
other commands until tick 16157. Both teams send it, so the sender set alone
does not identify the conceding team.

The payload `u32` is the winner, as a 1-based index into the engine's
PlayerManager list:

1. Issuer `0x41df9a`, once the concede vote has passed (`0x41e0c9`, read as
   "every eligible ally of the conceder agreed", medium): `0x41e318` picks the
   first PlayerManager entry that is not the conceder, has player flags
   `+0x14`/`+0x15` clear and is on a different team (relation 1 from
   `0x815070`). `0x41e38c` returns its list index plus one
   (`lea eax,[edi+0x1]` at `0x41e3d9`). The command is sent with
   `WorldDoCommandPlayer(0x61)` at `0x41e00d`.
2. Handler `0x7eac80`: `0x7ed330` reads the `u32` from the custom payload
   (stream over command `+0x40`/`+0x44`) and passes it to
   `PlayerManager::SetConcessionWinningPlayerSlot` (import `0xf88420`), which
   stores it at world `+0x1a4`.
3. The SCAR rules `WinCondition_ConcessionSuccessful*` call
   `World_SetTeamWin(World_GetPlayerAt(World_ConcessionWinningPlayerSlot()))`.
   `World_GetPlayerAt` (`0x8d1550`) subtracts one before indexing.

So the winning team of a conceded game can be read from the file: the
opcode 97 payload names the winning player, and `DATAINFO` gives its team.

Rule: **winner slot = payload − 1**, i.e. the PlayerManager list is in slot
order, not in `DATAINFO` order. Ground truth: in `temp.rec` the payload is 1,
which gives slot 0 (Kata VIII, team 1), and the user confirmed that team 1
won that game. The `DATAINFO` order (5, 3, 4, 2, 1, 0) would have given
slot 5 on team 0, which is contradicted. This rests on one replay. A conceded
game won by the other team, or with a different slot layout, would confirm
it further.

Still open: why slot 4 (on the conceding team 0) sends no opcode 97. The
mechanism is static (disassembly of the analysed build), not a runtime trace.

## Receive-side dispatch

There is no single opcode switch. Dispatch splits by receiver kind:

* `SquadController::AcceptCommand` (SimEngine `0x10025b50`, exported) walks
  the controller's extension list (`[this+0x128]`) and virtual-calls each
  extension's vtable slot 2 (`+0x8`) with the command until one accepts;
  `EntityController::AcceptCommand` (`0x100147b0`) is the entity analogue.
  Game extension classes (names from RTTI) switch on `[command+0]`:
  `ProductionQueueExt` `0x88c390` accepts 3/4/5/11/15/16, `CombatExt`
  `0x87dab0` accepts 0/1/9/24/25/37, `AbilityExt` `0x9e9790` accepts 13,
  `SquadProductionQueueExt` `0x9e7040` accepts 49/50/51/63/64,
  `SquadAbilityExt` `0x8b7ab0` accepts 53, `SquadCombatStanceExt` `0x985110`
  accepts 65/71, `SquadItemSlotExt` `0x9e9de0` accepts 60.
* Squad movement/combat commands are claimed by the `SquadState*` classes via
  vtable slot 6: Stop `0xa0de20` (43), Move `0x9a5550` (44 and 69),
  Construction `0x9b52c0` (46), Capture `0xa0b3e0` (47), Combat `0x985860`
  (48, 68), AttackMove `0xa0e560` (52), Ability `0x9710b0` (53), Load
  `0xa0bcb0` (56), HoldUnload `0xa0cb80` (58), Unload `0xa0d5b0` (59),
  Retreat `0x878710` (61), Patrol `0xa0f800` (66), Melee `0xa10ea0` (70, 48),
  Jump `0xa12ff0` (72), HunkerDown `0xa0add0` (73), Pickup `0xa138c0` (74),
  Plan `0xa14110` (75), Worship `0xa14bf0` (76).
* Player commands go through a direct jump table in `ModPlayer` (vtable
  slot 3, function `0x7e7840`): `add $-0x4e` rebases at 78, bound `0x15`,
  table at `0x7e7a04`, one distinct case per PCMD 78–99 (only 79
  `ChangeAlliance` routes to the reject label `0x7e79f9`). For example 78 →
  `0x7e787a`, 85 → `0x7e790e`, 94 → `0x7e7992`, 96 → `0x7e79b6`.

Caution for future work: `ModSquadController` vtable slot 4 (`0x879630`) looks
like a command switch but is `OnEvent(SimEvent&)` — its switch is over the
event type, not the command opcode.

Not located as literal compares: receive handlers for opcodes 2
(`CMD_Destroy`) and 23 (`CMD_UnloadSquads`); their acceptance presumably sits
in controller base paths. The names are unaffected (enum sources above).

## Custom-tag codecs

Senders never call `WorldCommand::SetCustomDataRaw` (its only call site,
`0x41ecdb`, is the stream reader). Every sender stores the tag byte directly
(`mov byte [command+0x48], TAG`), serializes through a
`WorldCommandWriteStream`, and commits with
`WorldCommandWriteStream::SetCustomData`. A full-image scan for that store
finds exactly one writer per tag, which pins each layout and its issuing
opcode(s).

Three primitives cover every corpus payload (SimEngine):

* `PackEntity` (`0x10022a80`): `u32` LE runtime entity id (`Entity+0x2c`),
  0 when null.
* `PackSquad` (`0x10022c50`): `u16` LE runtime squad id (`Squad+0x40`),
  0 when null.
* `PackTarget` (`0x10022780`): `u8 target_type` (bit 7 = a flag from
  `Target+0xc`), then by type via the jump table at `0x10022884`: type 2 →
  12-byte position (3 floats), type 3 → `PackEntity`, type 4 → `PackSquad`,
  type 5 → raw 12-byte position, other types → nothing. Wire sizes on the raw
  adapter: none = 1, squad = 3, entity = 5, position = 13 — exactly the
  observed tag-1 lengths 1/3/5/13.

Per-tag layouts (writer site; paired opcode(s) by caller cross-reference; all
corpus `(tag, length)` pairs are reproduced by these):

| tag | engine codec / writer | layout (wire order) | opcode(s) | corpus check |
| ---: | --- | --- | --- | --- |
| 1 | `SingleTargetData` (`0x7ed544`) | Target | generic attach | `(1,1/3/5/13)` |
| 2 | `SingleTargetBoolData` (`0x9413df`) | Target + `u8` | 48 (`0x985f33`) | — |
| 3 | `DualTargetData` (`0x9412df`) | Target + Target | 44, 23, 58 | `(3,26)` = 13+13 |
| 5 | `FlagData` (`0x7ed4c4`) | `u32` | 3/15/49/50/51/60/65/71… | `(5,4)` |
| 6 | `FlagTargetData` (`0x9411cf`) | `u32` + Target | 14, 49 | `(6,17)` = 4+13 |
| 7 | `FlagTargetBoolData` (fn `0x941340`) | Target + `u32` + `u8` | 52 (caller `0x9395d9`) | no corpus instances |
| 8 | `FlagDualTargetData` (`0xb7fe4f`) | `u32` + Target + Target | SCAR relays | `(8,30)` |
| 12 | `SquadCustomData` (`0xa02c6f`; Pack SimEngine `0x10021ba0`) | `u16` squad id + 12-byte position | 96 | `(12,14)` |
| 15 | local `0x812a40` via `0x941468` | `u32` EBP id + position + position + `u8` + entity `u32` + squad `u16` | 78 | `(15,35)` = 4+12+12+1+4+2 |
| 16 | same writer via `0x9414f8` | as tag 15 | 89 | `(16,35)` |
| 17 | same writer via `0x941588` | as tag 15 | 90 | — |
| 18 | fn `0x96c1a0` | Target + `u32` | 44, 52 | `(18,17)` = 13+4 |
| 19 | fn `0xa71fd0` | Target + Target + `u32` | 52 | `(19,30)` = 13+13+4 |
| 25 | fn `0x8b8a10` | `u32` ability id + `u8` | 53, 85 | `(25,5)` |
| 26 | local `0x8132d0` | `u32` id + `u8` + `u8` + Target | 13, 53, 85 | `(26,9/11/19)` = 6+3/5/13 |
| 27 | local `0x813b20` | `u32` id + `u8` + `u8` + 2×Target | 13, 53, 85 | `(27,32)` = 6+13+13 |
| 28 | local `0x8133d0` | `u32` id + `u8` + `u8` + 5×Target | 13, 53, 85 | `(28,13/23/59/71)` |
| 29 | local `0x8134e0` | `u32` id + `u8` + `u8` + 10×Target | 13, 53, 85 | `(29,40/88)` |
| 39 | fn `0x9fd890` | 12-byte position + `u32` | 94 | `(39,16)` = 12+4 |

The variable tag-28/29 lengths are the sum over the fixed target count: for
example `(28,71)` = 6 + 5 positions (5×13), `(28,13)` = 6 + 1 squad target +
4 empty targets (3 + 4×1), `(29,40)` = 6 + 2 positions + 8 empty.

### Blueprint namespaces for the lead `u32`

The PBG path-to-type mapping registered at `0x81bc10` includes type 0 =
`EBPs\` (default), 1 = `SBPs\`, 5 = `Upgrade\`, 6 = `Ability\`. Receive-side
`PropertyBagGroupManager::GetGroup(type, id)` calls tie the command values to
namespaces:

* opcodes 3/4 (tag 5): `GetGroup(1, u32)` at `0x88c4a7` — **squad blueprint
  (SBP) index**;
* opcodes 15/16 (tag 5): same call site, inner switch selects type 5 —
  **Upgrade index**;
* opcode 50 (and 64; tag 5): `GetGroup(5, u32)` at `0x9e75d8` — **Upgrade
  index** (the old "wargear purchase" values are Upgrade-namespace indices);
* opcodes 49/63 (tags 5/6): `GetGroup(0, u32)` at `0x9e70aa` — **entity
  blueprint (EBP) index** of the reinforced model;
* opcodes 53/85 (tag 25): `GetGroup(6, u32)` at `0x8b7b61` — **Ability
  index**. Tags 26–29 share the writer family and the lead-`u32`-plus-flags
  shape ("supported" for the same namespace; the direct `GetGroup` proof was
  recovered for tag 25);
* opcodes 78/89/90 (tags 15/16/17): `GetGroup(0, u32)` at `0x7e8546` —
  **EBP index** of the structure to construct. This resolves the previously
  unexplained lead field of the 35-byte form, and the 4+2 tail is
  `PackEntity`/`PackSquad` of the builder entity and squad (0 when absent);
* opcodes 5/51 (tag 5): **not** a blueprint — a runtime cancellation token,
  the 1-based per-producer enqueue ordinal (see the cancellation section
  below).

These are separate per-type catalogues: the same integer under different
opcodes names unrelated blueprints, which definitively resolves the old
"value 191 appears as two different items" observation.

## Verdict for every original opcode row

The “observed shapes” column comes from the six-file inventory (five checked
samples plus the local sixth replay). `none` means custom type `0xff`. The
“engine name” column comes from the recovered enum above; names marked `(s)`
are supported (array position only), all others are Lua-registration proven.
The names are build facts for the pinned executables, not a cross-version
contract.

| opcode | engine name | Original README label | Observed shapes | Disposition |
| ---: | --- | --- | --- | --- |
| 1 | `CMD_Stop` | Ability on placeable object | not observed | old label wrong; entity stop command with live issuers (e.g. `0x8d13ce`, `0x9df621`) that never fired in the corpus |
| 2 | `CMD_Destroy` | — | `none` | enum proven; receive handler not individually located |
| 3 | `CMD_BuildSquad` | Build unit | `(5,4)` | confirmed; the `u32` is an SBP index (`GetGroup(1, ·)`) |
| 5 | `CMD_CancelProduction` | Cancel unit or wargear | `(5,4)` | confirmed; the `u32` is the per-producer enqueue ordinal (see cancellation) |
| 9 | `CMD_Attack` | Attack from placeable object | `(1,3)` | confirmed entity attack (CombatExt `0x87dab0`, HoldExt `0x962ea0`) |
| 11 | `CMD_RallyPoint` | Set rally point | `(1,3)`, `(1,13)` | confirmed (ProductionQueueExt tag-1 path) |
| 13 | `CMD_Ability` | — | `(25,5)` | confirmed (AbilityExt `0x9e9790`; shares the tag 25–29 writers with 53/85) |
| 15 | `CMD_Upgrade` | Upgrade building | `(5,4)` | confirmed; the `u32` is an Upgrade index (`GetGroup(5, ·)`) |
| 23 | `CMD_UnloadSquads` | Exit building | `none` | name proven (enum + tag-3 issuer `0xa75171`); old label compatible but handler not individually located |
| 43 | `SCMD_Stop` | Stop move | `none`, `(1,1)` | confirmed (SquadStateStop `0xa0de20`) |
| 44 | `SCMD_Move` | Move | `(1,3)`, `(1,5)`, `(1,13)`, `(3,26)`, `(6,17)`, `(8,30)`, `(18,17)` | confirmed (SquadStateMove `0x9a5550`; prior construction trace) |
| 47 | `SCMD_Capture` | Capture point | `(1,5)` | confirmed (SquadStateCapture `0xa0b3e0`) |
| 48 | `SCMD_Attack` | Attack | `(1,3)`, `(1,5)`, `(1,13)` | confirmed (SquadStateCombat/Melee) |
| 49 | `SCMD_ReinforceUnit` | Reinforce unit | `(5,4)` | confirmed; the `u32` is an EBP index (`GetGroup(0, ·)`) |
| 50 | `SCMD_Upgrade` | Purchase wargear | `(5,4)` | confirmed; the `u32` is an Upgrade index (`GetGroup(5, ·)`) |
| 51 | `SCMD_CancelProduction` | Cancel wargear purchase | `(5,4)` | confirmed; the `u32` is the per-producer enqueue ordinal |
| 52 | `SCMD_AttackMove` | Attack move | `(1,13)`, `(6,17)`, `(19,30)` | confirmed (SquadStateAttackMove `0xa0e560`; tag-19 writer caller `0xa6ffe0`) |
| 53 | `SCMD_Ability` | Ability on unit | `(1,3)`, `(1,5)`, `(1,13)`, `(25,5)`, `(26,9)`, `(26,11)`, `(26,19)`, `(27,32)`, `(28,13)`, `(28,23)`, `(28,71)` | confirmed (SquadAbilityExt `0x8b7ab0`; tag-25 `u32` is an Ability index) |
| 56 | `SCMD_Load` | Enter building or vehicle | `(1,3)`, `(1,5)` | confirmed (SquadStateLoad `0xa0bcb0`) |
| 58 | `SCMD_UnloadSquads` | Exit vehicle | `none` | confirmed (SquadStateHoldUnload `0xa0cb80`) |
| 61 | `SCMD_Retreat` | Retreat | `none`, `(1,5)` | confirmed (SquadStateRetreat `0x878710`; prior input-key trace) |
| 70 | `SCMD_Melee` | Force melee | `(1,1)`, `(1,3)`, `(1,5)` | confirmed (SquadStateMelee `0xa10ea0`) |
| 71 | `SCMD_SetWeaponStance` | Toggle stance | `(5,4)` | confirmed (SquadCombatStanceExt `0x985110`, requires tag 5) |
| 78 | `PCMD_ConstructStructure` | Place building | `(15,35)` | confirmed (ModPlayer case `0x7e787a`; lead `u32` is an EBP index) |
| 85 | `PCMD_Ability` | Global ability | `(25,5)`, `(26,9)`, `(26,19)`, `(27,32)`, `(28,59)`, `(28,71)`, `(29,40)`, `(29,88)` | confirmed (ModPlayer case `0x7e790e`) |
| 89 | `PCMD_ConstructFence` | Unknown | `(16,35)` | confirmed (tag-16 writer caller `0x93b127`); the two positions are the fence endpoints |
| 94 | `PCMD_Ping` | Unknown, source `0x0` | `(39,16)` | confirmed (ModPlayer case `0x7e7992`; tag-39 writer caller `0xaedb8e`); payload = position + `u32` |
| 96 | `PCMD_ResurrectHero` | Unknown, source `0x0` | `(12,14)` | confirmed (ModPlayer case `0x7e79b6`; tag-12 payload = squad `u16` + position) |
| 98 | `PCMD_SetPreferredRetreatTarget` `(s)` | Unknown | not observed | name supported (array slot `0x12bbea8`); live issuers `0x899da7`, `0xb5f93f`; never fired in the corpus |

The old names came from small hand-labelled examples; most turn out to be
close to the engine names, with the notable corrections that opcode 1 is the
entity stop command (not an ability), 50 is the squad upgrade purchase (the
"wargear" values live in the `Upgrade\` catalogue), and 89/94/96/98 now have
engine names. The engine names should still be emitted alongside the numeric
opcode and the build/mod checksum, not instead of them.

## Why the old fixed rows fail

The old rows 1--19 were shifted by treating the first byte of the wire length
as an action field. After the inclusive `u16`, only the opcode and sender byte
have fixed positions. The next four bytes are copied reserved context; the
receiver field has one of four widths (single receiver or three count widths),
and the custom field is variable length. Consequently:

* old row 2 is the wire opcode and is structurally confirmed;
* old row 3 is the sender/process flags byte, not a player-location ID;
* old rows 4--7 are one four-byte reserved context value. Its identity-getter
  plus per-proxy-counter origin is now proven, but the getter's game semantic
  name remains unresolved; and
* old rows 8 onward cannot be assigned fixed meanings because receiver count,
  receiver list width, custom tag, and custom length vary by command.

The large named examples below this audit are preserved as historical sample
observations. Their names and values should be read through the corrected wire
layout above; rows whose original byte columns do not match the command length
are incomplete examples, not proof of a different format.

The old `(custom_type, custom_length)` observations that survive strict
decoding are `(1,1)`, `(1,3)`, `(1,5)`, `(1,13)`, `(3,26)`, `(5,4)`, `(6,17)`,
`(8,30)`, `(12,14)`, `(15,35)`, `(16,35)`, `(18,17)`, `(19,30)`, `(25,5)`,
`(26,9)`, `(26,11)`, `(26,19)`, `(27,32)`, `(28,13)`, `(28,23)`, `(28,59)`,
`(28,71)`, `(29,40)`, `(29,88)`, and `(39,16)`. The old `(12,4)` and
`(32,84)` claims were not reproduced as strict custom type/length pairs in
the six-file corpus and remain incomplete observations.

## Archive and semantic audit corrections

The archive audit checked the six replay identities against the pinned DOW2 and
SimEngine builds. It supports several context labels while keeping the actual
attribute name lookup unresolved:

* Opcodes 3, 5, 15, 49, 50, and 51 all use the observed type-5/length-4
  envelope in the corpus. Their payload is a full little-endian `u32`; it is
  not a one-byte item ID. The Tyranid sample `05 01 00 00` is `261`, and the
  Space Marine/Ordo Malleus tier sample `be 01 00 00` is `446`, not `190`.
  The handler-level namespace of each value is now proven (SBP index for 3/4,
  Upgrade index for 15/16/50/64, EBP index for 49/63, enqueue ordinal for
  5/51); turning an index into a display name still requires the matching
  build's attribute data.
* Opcode 53 uses the observed type-25/26/27/28 forms and opcode 85 uses the
  observed type-25/26/27/28/29 forms. DOW2 readers at `0x814350` and
  `0x814400` branch on custom tags 25 and 26 and then consume command-owned
  payload pointers and lengths. The tag layouts are now recovered writer-side
  (see the custom-tag codec table): an ability-blueprint `u32` (proven an
  `Ability\` index for tag 25 via `GetGroup(6, ·)` at `0x8b7b61`), two flag
  bytes for tags 26–29, and 1/2/5/10 packed targets. Which ability a given
  index names still depends on the build's attribute data.
* Opcode 78's type-15/length-35 payload is emitted for a base receiver. The
  type-15 writer at `0x941440` calls `0x812a40`; that helper serializes the
  observed 35-byte form as `4 + 12 + 12 + 1 + 4 + 2` bytes. The previously
  open lead and tail fields are resolved: the lead `u32` is the structure's
  EBP index (receive-side `GetGroup(0, ·)` at `0x7e8546`), the two 12-byte
  pieces are position vectors, and the 4+2 tail is the builder's packed
  runtime entity `u32` and squad `u16` (zero when absent). The single `u8`
  between them remains an unnamed flag.
* Cancellation (opcodes 5 and 51, and 93 on the player side) is a production
  queue operation, not a blueprint reference: the enqueue routine `0x871790`
  increments a per-queue counter (`[queue+0x4c]`) and stores it in the queue
  record (`[record+4]` via `0x870d40`); the cancel routine `0x871ae0` walks
  the records comparing `[record+4]` against the command's `u32`. The value is
  therefore the 1-based enqueue ordinal within that one producer's queue.
  Corpus check (3v3.rec, squad receiver `0x2000c356`): six opcode-50 enqueues
  are ordinals 1–6 and the three cancels carry tokens 1, 3, and 4. The
  earlier queue walkers `0x437d32`/`0x43834c` with key resolver `0x41ea96`
  belong to a different layer (`WorldCommandManager` bookkeeping); the replay
  cancel `u32` never reaches them.
* Within-type blueprint index order is proven for RBF builds: the loader
  (`LoadAllRBF`, SimEngine `0x10006470`) sorts each type's `FindFiles` result
  with `FileSet::StrAlpha`, which is `_stricmp(a, b) < 0` — case-insensitive
  alphabetical order of the full virtual path; archive TOC order is
  irrelevant. The RB2 branch (`LoadAllRB2`, `0x100061e0`) has no sort and
  creates groups in RB2 entry order.

The archive names and old “Item ID” table remain annotations rather than a
cross-build catalogue: the indices are clean per-type catalogue positions,
but the catalogue is a function of the exact mounted file set. A name-to-ID
claim requires the attribute archives of the build that recorded the replay,
keyed by the replay header's data checksum. The installed Elite archives match
`temp.rec` and one further replay with `mod_chksum` 4156674300, where the
name-to-index mapping is now verified (1,092 commands, 100% race-consistent;
rule in `docs/replay-format.md`). The same overlays belong to other builds for
`3v3.rec` and `recs/*` (whole annotated race blocks match at constant offsets, which
is exactly the signature of an alphabetical index after files were added).

## Evidence limits

The analysed binaries prove the byte-level envelope, the codecs, the opcode
enum, and the per-opcode namespace of each custom `u32` — all for this build.
A type-5 value is only a blueprint index under the opcodes whose handlers
resolve it as one (3/4, 15/16, 49/63, 50/64); under 5/51 it is a runtime
ordinal, and other opcodes give it other meanings. Receiver instances remain
runtime IDs that are not stable across recordings. Matching an index to a
display name across game or mod versions requires the replay's data checksum
and the matching attribute archives (stricmp path order for RBF builds, RB2
entry order otherwise), plus reconstruction of the session's runtime group
creation order for runtime IDs.

Remaining unresolved payload bytes: the `u8` flag in tags 15/16/17 (offset
0x1c of the payload), the two flag bytes in tags 26–29 and the single byte in
tag 25 (the tag-25 byte feeds a `WorldCommand::IsTrusted` check at
`0x8b7b3a`), and the trailing `u32` of tags 18/19/39. They are serialized and
parsed, but no statically reachable consumer names their meaning.

See the command-codec research for the broader custom-tag dispatch inventory
and the replay-format research for runtime ID allocation evidence when those
documents are available in the repository.

## Native command-construction audit

The generic serializer cannot name an action, but the analysed DOW2 executable
also contains callers that construct commands with literal opcode arguments.
This audit predates the enum recovery above and is retained for its call-site
addresses and codec context; the per-row conclusions are updated to the
recovered names. The byte is still not a stable enum across builds.

| opcode | construction evidence in the analysed DOW2 build | conclusion for the old label |
| ---: | --- | --- |
| 23 | `0xa7515e..0xa752ba` and `0xaf4913..0xaf49fb` call `WorldDoCommandEntities`/`WorldDoCommandEntity` with `SingleTargetData` or `DualTargetData`; target fields are reset from squad/entity values. | **Named:** `CMD_UnloadSquads` (enum-proven). The target-bearing entity construction matches; the old “exit building” reading was close but receiver-side, not building-specific. |
| 43 | `0x8d13ae` emits the byte for either the owner squad or entity after clearing an owner-pointer queue; `0x96b921` emits it after opcode `0x2a` and immediately changes entity state. Additional squad callers occur at `0x9dc409`, `0x9e6334`, `0xa71e7a`, and `0xb75e54`. | **Named:** `SCMD_Stop` (enum-proven; accepted by SquadStateStop `0xa0de20`). |
| 44 | At `0x7eab49..0x7eabd1`, the caller prepares a `Vector3f`, invokes imported `GetSquadStateProcessor` and `SquadStateProcessor::DefaultCommand`, then pushes squad, player, `false`, and literal `0x2c` into `WorldDoCommandSquad`; a returned command is queued with the same vector. Other callers occur at `0xa7003f` and `0x9de480`. | **Named:** `SCMD_Move` (enum-proven; accepted by SquadStateMove `0x9a5550` together with 69 `SCMD_Face`). The opcode remains version-specific. |
| 48 | `0x985f20`, `0x9a81f8`, `0x9cadf7`, `0x9dc3cb`, `0xa10fd5`, `0xa7024b`, and `0xb7081d` emit the byte through squad constructors. The surrounding paths build target data or squad groups, but no native name “attack” is retained. | **Named:** `SCMD_Attack` (enum-proven; accepted by SquadStateCombat `0x985860` and SquadStateMelee `0xa10ea0`). |
| 52 | `0x9395c6` emits the byte after constructing a squad group and `FlagTargetBoolData`; the follow-up codec writer at `0x941340` writes custom tag `7`. Similar target paths occur at `0xa0e515` and `0xa6ffcd`. | **Named:** `SCMD_AttackMove` (enum-proven; accepted by SquadStateAttackMove `0xa0e560`). The tag-7 writer `0x941340` on this path never fires in the corpus. |
| 56 | `0x9ca599`, `0x9dbbf5`, and `0xaf4cf4` emit the byte after constructing target data from entity/squad objects. | **Named:** `SCMD_Load` (enum-proven; accepted by SquadStateLoad `0xa0bcb0`), so the old “enter building or vehicle” label was essentially right. |
| 58 | `0xaf4b2c`, `0xaf4b89`, `0xaf4be4`, and `0xaf4c1b` emit the byte for squad groups after `DualTargetData`/`SingleTargetData` construction. | **Named:** `SCMD_UnloadSquads` (enum-proven; accepted by SquadStateHoldUnload `0xa0cb80`). |
| 61 | `0xb5f0e7` checks input key `0x85`, filters eligible squads, and emits the byte with `WorldDoCommandSquads`; target/squad paths also occur at `0x9de5f2` and `0x9e04ac`. | **Named:** `SCMD_Retreat` (enum-proven; accepted by SquadStateRetreat `0x878710`). Preserve the numeric opcode and raw fields regardless. |
| 70 | `0xb70cd3` filters squads by property/state predicates and emits the byte with `WorldDoCommandSquads`. | **Named:** `SCMD_Melee` (enum-proven; accepted by SquadStateMelee `0xa10ea0`), so the old “force melee” label was essentially right. |
| 71 | `0xb76ae1` checks `SquadController::QI(..., 8)` and a state helper for each squad before emitting the byte with `WorldDoCommandSquads`. | **Named:** `SCMD_SetWeaponStance` (enum-proven; accepted by SquadCombatStanceExt `0x985110` with a tag-5 stance value). |

This audit searched the direct `WorldDoCommand*` imports, the
`WorldCommand::DoCommand`/`BaseState::ProcessCommand` call chain, and the
`WorldCommandWriteStream::SetCustomData` callers in the pinned DOW2 build.
The game passes the opcode into generic constructors, then dispatches through
virtual controller/state methods, so no single receive switch covers all
values — but the enum itself survived in the SCAR registration, the name
arrays, and the literal issuer sites documented above. The table here records
the per-opcode constructor evidence; the recovered names remain scoped to the
analysed build, not a universal claim for every other game or mod version.
