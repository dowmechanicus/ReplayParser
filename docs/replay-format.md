# Dawn of War II replay format and identifiers

This note records the parts of the `.rec` format that are supported by static
analysis of the original x86 game and by boundary checks over the five replay
samples in this repository. It intentionally labels inference and open work.
The replay is a recording of simulation messages; it is not a self-contained
database of the unit and ability names that were active when it was recorded.

The dependency-free [`tools/replay_inspect.py`](../tools/replay_inspect.py)
checks these boundaries without trying to assign names to unknown opcodes or
attribute values. It validates both Relic Chunky signatures, checks the
`FOLDPOST` sync-record count against the stream, decodes the chat payload
layout, and reports the `DATAINFO` player blocks:

```text
python3 tools/replay_inspect.py 3v3.rec recs/*.rec
```

The five checked samples contain 38,389 sync records, 20,434 commands, and 61
chat records. The checks are structural evidence, not a guarantee that every
future build uses every field in the same way.

## File creation and the fixed header

The writer identified in the supplied `DOW2.exe` writes the first 84 bytes as
follows. Integers in this table are little endian.

| Offset | Size | Meaning | Evidence |
| ---: | ---: | --- | --- |
| `0x00` | 4 | replay format/version (`0x2850` in the analysed build) | writer at `42b1..42b259` |
| `0x04` | 4 | data/mod checksum (value varies with the data set; e.g. `0xf7c1d0fc` in the Elite mod replay `temp.rec`). `0x7a0a30` is the code address of the writer's checksum routine, not a value | writer and validator |
| `0x08` | 4 | reserved, observed zero | writer |
| `0x0c` | 8 | ASCII `DOW2_REC` | writer |
| `0x14` | 64 | 32 UTF-16LE code units containing the date/time | writer |

The validator at `42b259` compares only the first byte of the magic in the
analysed build. A parser should still require all eight bytes, as the inspector
does. The validator also compares version and checksum.

The checksum is not a simple checksum of the mod label and it is not the file
MD5. The call chain at `7a0a30` obtains a sync token through
`PropertyBagGroupManager::GetSyncToken` (`81c1c0`) and builds a CRC over
`data:maps\\pvp\\_cameras\\*.*`. This is a data identity used by the game to
reject incompatible content; it is not enough to turn a replay into a
self-contained, cross-version representation.

After the fixed header the writer emits Relic Chunky data. The 36 bytes at
`0x54..0x77`, previously treated as an opaque prelude, are the standard Relic
Chunky v3 file signature. It is written by `Chunky::ConstructInternal`
(Filesystem.dll `0x10008de0`; the write-mode branch at
`0x10009113..0x100091d1` assembles exactly this block):

| Offset in block | Size | Value | Meaning from the writer |
| ---: | ---: | --- | --- |
| +0 | 16 | `"Relic Chunky\r\n\x1a\0"` | file magic |
| +16 | 4 | 3 | chunky format version |
| +20 | 4 | 1 | `PlatformType` constructor argument (1 = native byte order) |
| +24 | 4 | `0x24` | signature block size |
| +28 | 4 | `0x1c` | chunk header size |
| +32 | 4 | 1 | constant in this writer; not named further |

A normal Chunky header is 28 bytes, and its last three words are also named by
the writers (`Chunky::WriteFolderChunk` at Filesystem `0x10009210`,
`WriteDataChunk` at `0x10009480`, `WriteDataChunkName`/`WriteFolderChunkName`
for the named forms):

```text
char[8] name              # FourCC kind + FourCC id, e.g. FOLD+POST
u32     version
u32     payload_size
u32     name_length       # 0 in every replay chunk
u32     caller int        # 0 for FOLD chunks; the game passes -1 for DATA chunks
u32     zero
byte[]  name_length name bytes, then payload_size payload bytes
```

`FOLDPOST` at 120 contains `DATADATA` at 148 and ends at 180. Its single `u32`
payload is the **number of type-0 sync records** in the file: the writer at
DOW2 `0x42b7ff` emits it as 0 when recording starts (`0x42bb93`) and rewrites
it with the recorder's per-sync-record counter (`[recorder+0x4c]`, incremented
at `0x438266`) when the recording is finalized (`0x42bc4e`). The value matches
the sync-record census in all six distinct local samples (`3v3.rec` 16,779;
`recs/1.rec` 11,760; `recs/purchases_SM.rec` 5,988; `recs/upgrades.rec` 1,530;
`recs/unit_transformation_and_abilities.rec` 2,332; the external
version-10320 sample 13,651). A replay cut off before finalize may
therefore hold a stale 0 here.

The second "36-byte gap" at 180 is simply the same Chunky v3 signature again:
the recorder drives two `Chunky` writer instances over the same file, one for
the POST section and one for the scenario/metadata section (constructed at
`0x42b81a` and `0x42b6b2`), and each instance writes its own signature. The
scenario chunk `DATASDSC` (version 2008) starts at 216, followed by `FOLDINFO`
(written at `0x42b694`) containing `DATABASE` (version 9) and one `FOLDGPLY`
(version 4) per player slot with a `DATAINFO` (version 7) inside. `FOLDINFO`'s
declared payload ends the header region; the outer records begin there. The
individual `DATABASE` fields that are partly known (the random seed and the
start-location flag) are described in the next section; the `DATAINFO` layout
is below.

## Game setup seed (`DATABASE`)

The `DATABASE` (version 9) payload begins with three little-endian `u32`
words. Word 3 is the game's random seed, a `time()` value; for example
`0x6aca5bd0` is 2026-10-10 15:37:52 UTC.

| Word | Destination | Meaning | Tier |
| ---: | --- | --- | --- |
| 1 | `GameInfo[0]` | not established | open |
| 2 | `FUN_0041a21e` | not established | open |
| 3 | `GameInfo[0x35e]` (+0xd78) | random seed | proven (reader path) |

The seed path is `GameInfo_ReadFOLDINFO` (`0x0041af1a`), which stores word 3;
`PopulateGameInfo` (`0x0044742a`), which calls `srand(seed)` and logs
`PopulateGameInfo - random seed:[%u]`; and `GameInfo_ToGameSetup`
(`0x0041b39a`), which copies it to the game setup's `randomSeed` (+0x31c). The
corpus is consistent with this path. The seed is used by the player slot
assignment described under "Player slots (runtime player IDs)".

The `COLS` parameter in the `DATABASE` parameter list selects start locations:
0 means random start locations, and a nonzero value means fixed locations.
Tier: supported. The link from `COLS` to the shuffle's random flag is not traced
in code; it rests on the corpus validation (all fixed-location replays only
match when the shuffles are skipped).

## Player metadata (`DATAINFO`)

The per-player writer is DOW2 `0x41a9c1` (reader `0x41ab06`), called once per
player slot from the `FOLDINFO` writer `0x41ad12`. Strings are
`ByteStreamAdapter::WriteString` (u32 count followed by UTF-16LE code units
for the wide form, or bytes for the narrow form). The fields in wire order,
with their evidence tier:

| # | Wire type | Meaning | Tier |
| ---: | --- | --- | --- |
| 1 | wstring | player name | proven string write; corpus |
| 2 | u32 | player kind; the writer remaps `8 -> 7` at `0x41a9f9..0x41aa01` | wire proven; corpus values: 0 local human, 1 AI, 3 remote human, 5 observer, 7 empty slot |
| 3 | u32 | team index (0/1 in the samples; observers also carry one) | supported |
| 4 | string | race name (`space_marines`, ...; empty for observers) | supported |
| 5 | u64 | profile/relic ID; `0xffffffffffffffff` for empty and AI slots | supported |
| 6 | u32 | rank/level (16..60 observed; `-1` for empty slots) | supported |
| 7 | u32 | runtime player ID slot (`GameInfoPlayer` +0x9c); always 0 in recordings, see below | proven field role; 0 in all corpus blocks |
| 8 | u32 | 1 for the AI player in the corpus, otherwise 0 | observation |
| 9 | u32 | small per-race index 0..5, `-1` for observers/empty; plausibly the hero/commander selection | observation |
| 10 | u8 | 0/1 flag, meaning unresolved | observation |
| 11 | u32 | duplicate of field 6 (the writer serializes the same object field twice) | proven duplicate |
| 12 | 5 x u8 | appearance bytes; the last byte is 1 for every non-empty slot, the first four vary (colour indices) | observation |
| 13 | string | badge path (`data:art\badges\...`, or `badges:<hash>.png` for a custom badge) | supported |
| 14 | u32 | nonzero only alongside a custom `badges:` path; custom-badge hash/CRC | supported |
| 15 | wstring | army name (the army-painter title) | supported |
| 16 | u32 | race enum: 0 chaos_marines, 1 eldar, 2 imperial_guard, 3 orks, 4 space_marines, 5 tyranids (alphabetical); `-1` for observers/empty | supported, consistent across all corpus blocks |

All 48 player blocks in the six-file corpus (8 slots per file) decode under
this layout with zero leftover bytes. Numeric fields 8..10 and the colour
bytes are corpus observations, not writer-proven names.

Field 7 is a runtime value, not a stored setting. The reader
`GameInfoPlayer_ReadDATAINFO` (`0x0041ab06`) stores the fields after rank
(field 6) in this order: field 7 at `GameInfoPlayer` +0x9c, field 8 at +0x2c,
field 9 (hero) at +0x1c, field 10 (u8) at +0x20, and field 11 repeats the rank.
The writer is `GameInfoPlayer_WriteDATAINFO` (`0x0041a9c1`). The recorder
writes the header with `GameInfo_WriteFOLDINFO` (`0x0041ad12`, called at
recording start from `0x42b694`) before player IDs are bound, so field 7 is
always 0 in a recording. Save games restore it (`SaveGame_ReadGameInfo`,
`0x004138f1`).

Lobby members are placed in the block list by `GameInfo_FillPlayersFromLobby`
(`0x0044618c`). Each member goes to record index `team * 4 + n`, where `n` is
the number of members of that team placed so far, in lobby-list order. Team 0
therefore occupies blocks 0..3 and team 1 blocks 4..7, with empty slots (kind 7)
between them. No randomness is involved.

## Outer records and action commands

The stream after `FOLDINFO` is a sequence of records:

```text
u32 record_type       # 0 = simulation sync, 1 = chat
u32 payload_size
byte[payload_size] payload
```

A type-1 payload is a serialized `RecordedChatCmd` (writer `0x4379d6`,
reader `0x43792f`; the dwords are the payload object's `+0x0c`/`+0x10`/
`+0x14` fields). All 78 chat records in the six-file corpus decode exactly as:

```text
u32 inner_kind        # == 1, duplicate of the outer record type (78/78)
u32 inner_size        # == payload_size - 8 (78/78)
u32 + utf16le         # sender name (count then code units)
u32 sender_key        # 1000 + sender slot, the runtime player-ID namespace
u32 zero              # 0 in all corpus records; meaning open
u32 team_flag         # 0 or 1
u32 + utf16le         # message text
```

The inner kind/size pair is the message's own framing inside the serialized
buffer; the record writer adds the outer pair on top. The team flag is a
supported interpretation from content correlation: 0 on open chat ("gl hf",
"gg", all of a 1v1's banter) and 1 on team-directed tactical calls; the
recorded payload field it maps to is used in the dispatcher's visibility
logic. Playback feeds the same payload back into the live chat dispatcher
(`0x42757b` -> `0x429972`).

A type-0 payload is a serialized `GameMsgSyncCommand` network message: the
serializer at DOW2 `0x42aeea`/`0x42b041` carries the error string
`GameMsgSyncCommand::StreamMessageOut - error, cannot write [%s].`, and the
record is written once per simulated tick by
`SimulationController::SimulateInternal` (`0x437edb`, named by its own
diagnostics) through the record writer `0x42b2d3`. It starts with 17 bytes:

```text
u8  kind              # 0x20; the reader requires kind >> 4 == 2 (0x42ae4a)
u32 tick
u32 command_counter   # cumulative commands flushed by the emitting node
u32 sync_crc          # per-tick simulation-state CRC; 0 when absent
u32 bundle_count
```

The `command_counter` semantics are writer-proven for the single-source case:
`SimulationController::GenerateSyncCmd` (`0x437d32`, named by its binned-
commands diagnostic) samples the controller's counter at `+0x1c` into the
outgoing message, packs the pending command queue, and only then adds the
packed command count back to the counter (`0x437e34`). In the three
single-source repository samples the field equals the cumulative command count
before the record in 9,850 of 9,850 sync records. In multiplayer recordings
each record carries the value from its emitting peer's turn data
(`P2PNetworkSynchronization::GetSyncCommand`, `0x66d5a0`, copies tick/counter/
CRC verbatim from the received turn at `0x66d738`), so the stream value is not
globally monotonic; the exact multi-peer aggregation rule is open.

The `sync_crc` is the desync-detection checksum: at the end of every simulated
tick `SimulateInternal` calls `CRCBlockManager::CreateCRCBlock` (SimEngine
`0x1000f1b0`), which runs `World::GetSyncToken(CRCBlock&)` (World vtable
`0x1005b114`, slot `+0x80` = `0x1002b1b0`) over a fresh CRC block and returns
its accumulated CRC; the value is recorded with the following tick's sync
command. `GenerateSyncCmd` substitutes the sentinel `0xfffffffe` when a stall
flag is set (`0x437d77`; the network receive path tests for that sentinel at
`0x66d706`). Multiplayer recordings also interleave records with `tick == 0`
and `sync_crc == 0` that usually contain only empty bundles; they are recorded
sync commands for ticks where no local simulation step completed.

Each bundle is a serialized 0x18-byte element: eight raw header bytes, a
four-byte command-byte declaration, one echo byte, and the command bytes. The
echo byte is **proven** to be the low byte of the declaration (computed and
written at `0x42b0da..0x42b0e9`). The eight header bytes are serialized
verbatim and are zero in all 246,543 bundles of the six-file corpus; treat
them as reserved. Empty bundles carry a zero echo after their declaration;
non-empty bundles start their first command immediately after the echo.

Each command starts with an inclusive little-endian length. The length includes
the two length bytes and the command body:

```text
u16 command_size
u8  opcode
u8  sender_and_flags
u8[4] reserved/forwarded command context (often starts with a runtime player ID)
... receiver field ...
u8  custom_type       # 0xff means no custom field
... custom length and payload ...
```

The sender byte's low seven bits are the sender slot, `playerID - 1000`
(`Player::Base_PlayerID` = 1000); `0x7f` is the no-sender value.
`WorldCommand_Serialize` (`0x0041e8da`) writes `(playerID - Base_PlayerID) |
(flag << 7)`, and `WorldCommand_Deserialize` (`0x0041ea96`) reverses it. Its high bit is the command process
type (the analysed handler uses value 1 for the process form). The old parser
treated the first length byte as part of the action body, which shifted the
opcode and discarded the last payload byte. For example, the 18-byte command

```text
12 00 03 03 eb 03 00 00 10 00 77 0c 05 04 86 00 00 00
```

has opcode `3`, sender byte `3`, receiver word `0x1000770c` (entity ID
30,476), custom type `5`, and a four-byte custom value `134`.

The four bytes after the sender are copied by the serializer and should be
treated as reserved until their individual uses are established. In observed
commands they often split into a `u16` value around `1000 + sender_slot` and a
two-byte sequence. The legacy parser joined messages to actions using the low
byte of that reserved value; that is a compatibility join key, not proof that
the field is the command's semantic sender ID.

The per-sender sequence can restart at zero mid-game. The counter lives in a
per-player `CommandIssueProxy` (constructor at DOW2 `0xaf3170`; the issue
method at `0xaf30f0` writes the identity `u16` and the per-proxy counter and
increments it). The constructor has
exactly two call sites: per-player creation during game setup
(`0x80e6fe`) and a mid-game replacement (`0xac9930`, inside the
player-controller reassignment function `0xac9700`) that releases the old
proxy and installs an `EventGeneratorCommandProxy` (RTTI-named), i.e. the
hand-this-slot-to-an-event-generator/AI path, whose fresh counter starts at
zero. The one observed reset (`recs/1.rec`, sender 1, sequence 1037 -> 0
between ticks 8717 and 8944) matches that path: the sender goes silent for
227 ticks, resumes at sequence 0 with a setup-like burst, and its commands
thereafter arrive inside the remaining peer's own bundle. The replacement
path is binary-proven; attributing this specific replay event to it is a
supported interpretation.

Receiver words use a second, mixed-endian convention. A single receiver is a
big-endian `u32` whose high nibble is the receiver type and whose low 28 bits
are the instance ID:

| Type nibble | Meaning |
| ---: | --- |
| 0 | player |
| 1 | entity |
| 2 | squad |

For multiple receivers, the count is tagged in big-endian form: `0x40..0x7f`
for a one-byte count, `0x8000..0xbfff` for a two-byte count, or
`0xc0000000..` for a four-byte count. The receiver words following the count
are little-endian `u32` values. This deliberate mix of big- and little-endian
fields is one reason a byte dump is easy to misread.

The custom length is one byte for values below 128. Otherwise the high length
byte has its high bit set and the following byte is the low length byte. The
generic custom type `5` is `FlagData` and is read by the simulation as a
four-byte unsigned value. This does not prove that every type-5 value is a
purchase identifier.

The old table's apparent “unit ID” byte is the low byte of the receiver word,
not a blueprint ID. For example, the body

```text
03 00 e8 03 01 00 10 00 4a 65 05 04 bf 00 00 00
```

targets entity receiver `0x10004a65` (instance 19,045); the old byte would have
shown only `0x65`. Receiver bytes `10 00 78 ad` similarly carry instance
`0x78ad` (30,893), although the old byte is only 173. In `purchases_SM.rec`,
opcode-3 commands reuse entity instance `0x4a6f` while type-5 values vary, and
opcode-50 commands target squad instance 50,000 with different custom values.
These are separate command and runtime namespaces. A name-to-blueprint mapping
is proven only for the blueprint-bearing `u32` forms (SBP, EBP, `Upgrade\`,
`Ability\`) of the build with `mod_chksum` 4156674300 (see "Catalogue rule
verified for build 4156674300" below); it is not proven for receiver or runtime
IDs, nor for any other build.

## How the game assigns IDs

Several unrelated numbers were called “unit ID” in the earlier tables. They
must be kept separate.

### Blueprint and purchase values

Attribute records are `PropertyBagGroup` objects. Static analysis observes the
group type at `+0x4c` and its attribute ID at `+0x50`. The fixed DOW2 mapping
table at `0x81bc10` registers `EBPs\\` as type 0 and `SBPs\\` as type 1,
alongside 28 other path/type pairs. `SavePBG` writes an attribute path string;
it does not serialize the replay's runtime receiver ID.

The purchase command's custom value is therefore not automatically a runtime
entity ID. Some command handlers use queue slots or blueprint-related values;
the complete purchase-handler mapping is still open. The old action tables
should be read as historical observations of values in particular data sets,
not as a global ID catalogue.

### Runtime groups and receivers

The attribute/simulation group table stores groups in a vector per group type.
`CreateAndAddNewGroup` uses `vector[type].size()` before appending, and
`GetGroup(type, id)` indexes that per-type table. This is a per-type catalog or
group-table index; it must not be treated as a universal unit ID or as the
runtime squad receiver ID. An empty-group sentinel uses IDs at or above
`0x7fffffff`.

The analysed type getters identify player, entity, and squad as types 0, 1,
and 2. The observed ID bases are 1,000, 10,000, and 50,000 respectively. The
SquadFactoryImp counter starts from the squad base, ensures it is greater than
a supplied ID, and passes the result to the squad constructor. Destroying a
squad does not decrement that counter. This is the proven runtime squad-ID
sequence; constructor ID storage alone does not prove equivalent lifetime or
reuse rules for every player/entity path.

Consequently, two replays can give the same blueprint different receiver IDs
because the blueprint/table index, squad factory sequence, and command timing
are different namespaces and lifetimes.

### Player slots (runtime player IDs)

The engine player index is the sender slot, and the runtime player ID is
`1000 + slot`. Slots are assigned by `GameSetup_AssignPlayersToMapSeats`
(`0x0079df40`), which `GameSetup_AssignStartPositions` (`0x0079e880`)
dispatches. Its inputs are the lobby blocks with kind 0, 1, 3 or 4 (in block
order), the map's team layout, the game seed, and the `COLS` flag. The steps
are:

1. Create a `Random` `rng` with 4 rounds and call `rng.SetSeed(seed)`.
2. The lobby members are the blocks with kind 0, 1, 3 or 4, in block order.
   `GameInfo_ToGameSetup` appends them to the setup player array (stride 0x60:
   team at +0x10, lobby block at +0x1c). They are then grouped by lobby team
   (`GameSetup_GroupLobbyTeams`, `0x0079dc20`).
3. The engine's map positions are grouped by map team (`GameSetup_GroupMapTeams`,
   `0x0079dc90`). If the map defines no teams, the group of position `index` is
   `index / (count / expectedTeams)`.
4. If start locations are random (`COLS == 0`) (supported; see `COLS` above),
   the map-team order is shuffled
   with `random_shuffle`. For each lobby team `t` in ascending order, let
   `list = mapTeams[order[t]]`; if locations are random, `list` is shuffled too.
   Each member receives `list[p]`, where `p = block - 4t` if every member's
   block lies in `[4t, 4t + |list|)` (`GameSetup_UseLobbySlotPositions`,
   `0x0079dd90`), and otherwise `p` is the member's index within its team.
5. The member's engine player index is its sender slot. The member's `list[p]`
   value is the slot, and its runtime player ID is `1000 + slot`.

`random_shuffle` (`random_shuffle_Random`, `0x007a1ed0`) runs for `i` from 1 to
`n - 1` and swaps `a[i]` with `a[GetMax(i + 1)]`.

The `Random` class in Util.dll is the Numerical Recipes `psdes` generator with
state `{seed, index, rounds = 4}`. Each draw hashes `(seed, index)` and then
increments `index`. The round constants are C1 = `baa96887 1e17d32c 03bcdc3c
0f33d1b2` and C2 = `4b0f3b58 e874f0c3 6955c5a6 55a7ca46`. `GetMax(n)` returns
`hash % n`. With seed 1 and index 1 the output is `0x509c0c23`, matching the
published Numerical Recipes test value.

Evidence tier: the code path is proven by the Ghidra analysis, and the result is
supported by corpus validation. The rule reproduces 844 of 844 chat-identified
seats across 249 replays (stock and Elite, random and fixed locations), and
155 of 155 race-consistency checks across 26 Elite replays. Every validated game
had a full lobby. The contiguous map-team grouping in step 3 is the engine's
no-team fallback. It is unverified for partial lobbies.

### Why versions and mods mingle IDs

The attribute loader creates groups in archive/load order. The RB2 path makes a
first pass that creates groups in entry order and a second pass that reads their
payloads. The RBF path applies type mappings and enumerates filesystem
wildcards. For the analysed RBF build the within-type order is lowercase-fold
(`_stricmp`) alphabetical over the full path (see the catalogue rule below);
the RB2 path is entry order. Adding a blueprint, changing archive
ordering, or changing the startup scenario changes the numeric assignment
without changing the human-readable unit name.

The replay records numeric references plus a mod/data checksum; it does not
embed the complete attribute archive needed to resolve those references. A
parser using a different game version or mod can therefore decode a valid
number as another unit, and runtime entity/squad IDs can differ even when the
same blueprint is present. Cross-version comparison requires the matching
attribute data and the replay's checksum, plus a per-session reconstruction of
runtime group creation.

### Catalogue rule verified for build 4156674300

VERIFIED for the build with header `mod_chksum` 4156674300 (`0xf7c1d0fc`): for
each namespace, take every `.rbf` path under `simulation\attrib\<dir>\` across
the module's `[attrib:common]` archives (Elite_Attrib.sga,
Elite_Campaign_Attrib.sga, GameAttrib_orig.sga), lowercase it, deduplicate and
byte-sort it. The index is the position in that list.

| namespace | dir | count | opcodes (field) | evidence |
|---|---|---:|---|---|
| EBP (0) | `ebps` | 2591 | 49 (tag 5 u32), 78 / 89 (leading u32 of the 35-byte form) | 293/293 op 49, 79/79 op 78, 3/3 op 89 race-consistent |
| SBP (1) | `sbps` | 715 | 3 (tag 5 u32) | 61/61 race-consistent |
| Upgrade (5) | `upgrade` | 1751 | 15, 50 (tag 5 u32) | 18/18 op 15, 116/116 op 50 |
| Ability (6) | `ability` | 2399 | 53 (verified for tags 25 and 26), 85 (verified for tags 25, 26, 28 and 29); leading u32, resolver accepts tags 25..=29 | 508/508 op 53, 14/14 op 85 |

Evidence: `temp.rec` plus a second current-build replay, 1,092 resolved
commands across seven players (SM, CSM, IG, Tau), all race-consistent, none
out of range. Hero-specific purchases, abilities and turrets match each
player's lobby hero choice, and causal order holds (e.g. a wargear purchase
precedes the ability it unlocks).

Opcode status: 3, 15, 49, 50, 53, 78, 85 verified; 89 verified but thin (3
commands); 13 `CMD_Ability` is a hypothesis (seen only in the old-build
`3v3.rec`); 90 and 4/16/63/64 are documented in the audit only, never observed.
Global/commander abilities are opcode 85 and resolve into
`ability\pvp\race_*\globals\` (14/14); hero abilities are opcode 53 into
`ability\pvp\race_*\<hero>\`. There is no separate commander namespace.

Alternatives tested: archive entry order fails (e.g. a CSM player's SBP 111
becomes an Imperial Guard kasrkin squad). Uppercase-fold order differs at 3 of
197 distinct observed indices; ability 906 decides it (lowercase gives
`ig_stormtrooper_squad_infiltrate`, a toggled ability; uppercase gives a
passive that can never be issued), so lowercase fold is verified over uppercase
fold. Dropping Elite_Attrib.sga breaks the data, so the Elite archive is
required. Whether Elite_Campaign_Attrib.sga is included is not discriminated by
the data (its entries sort above every observed index): HYPOTHESIS that it is,
consistent with the module mount. Whether unmapped directories that sort after
`ebps\` are folded into type 0 is open; it would only change the EBP catalogue
length (out-of-range detection), not observed indices.

Caveats: these indices are build-specific. Older replays (`3v3.rec` 3722732858,
`recs/1.rec` 4277459924, the 2021 corpus files 2388867438) still resolve to
in-range but wrong names against this install, so no out-of-range warning
fires. `mod_chksum` cannot yet be computed from the archives; its association
with this install rests on the 100% fit. Display names (UCS) are stored in the
RBFs as plain `u32` ids; how they are read and resolved is documented in
[`game-data-formats.md`](game-data-formats.md).

## Proven versus unresolved

The following are demonstrated by the supplied executable/static analysis and
the boundary-checked samples:

* the 84-byte header, magic, version/checksum fields, Chunky framing including
  both v3 signatures and the chunk-header word names, outer record framing,
  sync/bundle/command lengths, the bundle echo byte (low byte of the declared
  size), receiver type tags, and the mixed receiver endianness;
* the sync payload being a serialized `GameMsgSyncCommand`, its command
  counter (writer-proven sample-then-add at `GenerateSyncCmd`) and its
  per-tick `World::GetSyncToken` CRC with the `0xfffffffe` stall sentinel;
* the `FOLDPOST`/`DATADATA` u32 as the finalized sync-record count;
* the `DATAINFO` wire layout and the player-kind writer remap `8 -> 7`;
* the chat payload layout (inner framing pair, name, `1000 + slot` key,
  flag dwords, text);
* the two `CommandIssueProxy` construction sites, including the mid-game
  `EventGeneratorCommandProxy` replacement that restarts the reserved
  sequence;
* the separation between blueprint `PropertyBagGroup` IDs, per-type
  group-table indices, and runtime squad receiver IDs; the three type bases;
  and the squad counter lifetime behavior;
* the `DATABASE` word 3 random seed and its path into `srand` and the game
  setup's `randomSeed`;
* the `DATAINFO` field 7 role as the runtime player ID slot, the storage order
  of fields 7..11, and the lobby block layout from `GameInfo_FillPlayersFromLobby`;
* the sender byte encoding `playerID - 1000` in `WorldCommand_Serialize` and
  `WorldCommand_Deserialize`;
* the player-slot assignment code path (`GameSetup_AssignPlayersToMapSeats`)
  and its `Random` and `random_shuffle` primitives, with the corpus validation
  described in its section;
* the five-sample totals reported by `replay_inspect.py`.

These remain open and should not be guessed by a parser:

* the multiplayer aggregation rule for the sync command counter (per-peer
  origin is proven; the recorded value scales with bundle count empirically);
* the eight reserved bundle-header bytes (zero in all 246,543 corpus bundles);
* the meaning of `DATABASE` words 1 and 2, `DATAINFO` fields 8..10 and the exact
  colour-byte roles beyond the corpus observations above;
* the chat payload's always-zero dword and the formal enum behind the team
  flag;
* the semantics of every opcode, receiver context, and custom type;
* the exact purchase/cancel handler mapping from blueprint/queue values to
  replay custom payloads;
* complete RB2/RBF ordering rules across all releases and mods;
* the slot-assignment rule for partial lobbies (the contiguous map-team
  grouping is an unverified fallback);
* a complete reconstruction of runtime group lifetimes from every command.

The checksum and format version should be exposed to callers, and numeric IDs
should be presented with their namespace and source build rather than treated
as stable names.

The command-specific custom-data codec and its remaining semantic limits are
catalogued in [`command-codec.md`](command-codec.md). The inventory helper
[`tools/replay_command_inventory.py`](../tools/replay_command_inventory.py)
counts observed opcode/custom combinations without assigning version-specific
gameplay names.
