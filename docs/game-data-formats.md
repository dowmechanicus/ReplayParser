# Game data formats: SGA content, RBF, field names and UCS

This note records the parts of the Dawn of War II data files that the parser reads to turn a
blueprint into a readable name. The evidence is a survey of the Retribution install
(`Elite.module`, three attrib archives) and two replays with `mod_chksum` 4156674300, which is the
installed build. The replay side (how indices become blueprint paths) is in
[`replay-format.md`](replay-format.md); the user-facing behaviour is in the README section
"Resolving blueprint names (`--module`)".

Labels: **VERIFIED** means checked against data, over the whole corpus where a count is given.
**HYPOTHESIS** means consistent with the data but not proven. **OPEN** means unknown. A "corpus"
below is the set of `.rbf` files in the three `[attrib:common]` archives (16,023 files) unless
stated otherwise. Counts come from the findings survey, not from a test in this repository, unless
the evidence column names a test.

## 1. SGA archives (version 5.0), reading file content

All integers are little endian. The TOC (table of contents) layout is VERIFIED on all three attrib
archives (`Elite_Attrib.sga`, `Elite_Campaign_Attrib.sga`, `GameAttrib_orig.sga`).

| Offset | Size | Field | Evidence |
| ---: | ---: | --- | --- |
| `0x00` | 8 | magic `_ARCHIVE` | VERIFIED |
| `0x08` | 2 + 2 | major = 5, minor = 0 | VERIFIED; the reader rejects other majors |
| `0xB0` | 4 | `data_start` (`0xC4` in all three archives) | VERIFIED |
| `0xB4` | 4 | `toc_offset`, absolute (the TOC is at the end, after the data) | VERIFIED |
| `0xB8..0xC4` | 12 | not used (`01 00 00 00`, `00 00 00 00`, 4 bytes that look like a hash) | meaning OPEN, not needed |

The TOC header is 24 bytes at `toc_offset`, with offsets relative to `toc_offset`: drive
(`u32` offset, `u16` count), folder, file and name tables in the same form. `Elite_Attrib.sga` has
1 drive, 511 folders, 4,533 files and 5,044 names. A folder record is 12 bytes.

### 1.1 File record (22 bytes)

Example, `sm_generator.rbf` (record 1223 at `0xA07C12` in `Elite_Attrib.sga`):

```text
fe e7 00 00 | f7 68 3f 00 | d9 09 00 00 | 19 17 00 00 | 58 f3 78 6a | 00   | 02
name_off      data_off      clen = 2521   ulen = 5913   mtime         flag   storage
```

`data_off` is relative to `data_start`: `0xC4 + 0x3F68F7 = 0x3F69BB`, where the bytes are
`78 da ad 98 ...` (a zlib header).

| Storage byte | Meaning | Evidence |
| ---: | --- | --- |
| 0 | stored (uncompressed) | HYPOTHESIS: never observed. All 16,027 entries have `clen != ulen`. A stored entry would need `clen == ulen`; the reader checks the length after reading. |
| 1 | zlib (RFC 1950, header `78 DA`) | VERIFIED: only `autoblueprints.scar` uses it |
| 2 | zlib (RFC 1950, header `78 DA`) | VERIFIED: every `.rbf` entry uses it |

Entries per archive (storage 2 / storage 1): `Elite_Attrib` 4,532 / 1, `Elite_Campaign_Attrib`
709 / 1, `GameAttrib_orig` 10,783 / 1. Any other storage byte is an error in the reader.

### 1.2 Per-file preheader (260 bytes), VERIFIED on 16,027 of 16,027 entries

The 260 bytes immediately before each file's data (`data_start + data_off - 260`):

| Relative offset | Size | Field |
| ---: | ---: | --- |
| 0 | 256 | basename, NUL padded, Latin-1 (`sm_generator.rbf\0...`) |
| 256 | 4 | CRC-32 (IEEE, as zlib `crc32`) of the uncompressed data (`0d b4 d9 1c` = `0x1CD9B40D` for `sm_generator.rbf`) |

Files are packed back to back: the gap between consecutive entries is exactly 260 bytes, the first
entry has `data_off` = 260, and the last entry ends exactly at `toc_offset`.

### 1.3 Extraction recipe

1. Seek to `data_start + data_off` and read `clen` bytes (the reader adds `data_start` when it
   parses the TOC and checks `data_offset + clen` against the file length).
2. For storage 1 or 2, inflate as a zlib stream (RFC 1950, header and adler32) with an output
   limit of `ulen`.
3. Check that the result is exactly `ulen` bytes.

The survey validated all 16,027 entries this way (zlib OK, length OK, CRC OK). The parser does
**not** check the preheader CRC; it relies on the zlib adler32 and the length check.

## 2. Which copy of a path wins

`Elite.module [attrib:common]` lists `folder.01` `Elite\DataAttrib`, `folder.02`
`GameAssets\DataAttrib` (both absent from the install), then `archive.01` `Elite_Attrib.sga`,
`archive.02` `Elite_Campaign_Attrib.sga` and `archive.03` `GameAttrib_orig.sga`.

`.rbf` paths overlap between the archives. By the sets of archives that hold a path (Elite,
Campaign, Orig): only Orig 8,539; only Elite 2,322; Elite and Orig 2,201; only Campaign 658;
Campaign and Orig 41; Elite and Campaign 7; all three 2. Of the 2,201 Elite and Orig pairs, 2,183
differ in content.

**Rule: the first source in module order wins**, so a lower `archive.NN` beats a higher one. The
parser keeps the first insertion for each lowercased path (`parser_lib/src/attrib.rs`).

| Claim | Status | Evidence |
| --- | --- | --- |
| `archive.01` (Elite) beats `archive.03` (Orig) | VERIFIED | replay causality, below |
| `archive.02` (Campaign) ranks between them | HYPOTHESIS | the same rule applied; Elite vs Campaign (9 paths) and Campaign vs Orig (43 paths) are not discriminated by data. None of these 52 paths is under `pvp\`, so they cannot affect multiplayer replays. |
| Loose folders rank before the archives | HYPOTHESIS | matches the module's listing order. Neither folder exists in the analysed install, so this is OPEN in practice. The parser puts folders first. |

Causality evidence for Elite over Orig (`temp.rec`):

- Slot 1 casts global ability 960, `pvp\race_marine\globals\sm_spawn_drop_pod` (`PCMD_Ability`, tick
  5606). Squad instance 50029 first appears at tick 5711 and afterwards receives only `sternguard\*`
  abilities (`sm_toggle_ammo_hellfire`, `...vengeance`, `sm_and_they_shall_know_no_fear`) and an
  `SCMD_ReinforceUnit` with ebp `sm_sternguard_veteran`. Slot 1 never builds a sternguard squad
  itself (its `CMD_BuildSquad`s are tactical, assault, devastator and librarian).
- The `Elite_Attrib` copy of `sm_spawn_drop_pod.rbf` has
  `ability_bag\action_list\start_target_actions\delay\delayed_actions\spawn_squad\squad_name =
  sbps\pvp\race_marine\troops\sm_sternguard_veteran`. The `GameAttrib_orig` copy has no
  `spawn_squad` action. So the game ran the Elite copy.
- Supporting UCS evidence: in 28 differing pairs the Elite copy points at an `Elite.ucs` id and the
  Orig copy at a `DOW2.ucs` id (for example `ig_commissar_lord` 10001040 "Lord Commissar (Hero)"
  against 9159086 "Commissar Lord (Hero)"; `sm_spawn_drop_pod` 10001497 "Drop Pod Sternguard
  Veterans" against 9112951 "Drop Pod").

All 197 blueprints referenced by the two surveyed replays resolve to the `Elite_Attrib` copy.

## 3. RBF (`RBF V0.1`)

### 3.1 Header (40 bytes), VERIFIED on all 16,023 `.rbf` entries

| Offset | Size | Field | Rule |
| ---: | ---: | --- | --- |
| `0x00` | 8 | magic `RBF V0.1` | |
| `0x08` | 4 | `tables_off` | always `0x28` |
| `0x0C` | 4 | `table_count` | |
| `0x10` | 4 | `index_off` | `tables_off + 8 * table_count` |
| `0x14` | 4 | `index_count` | |
| `0x18` | 4 | `entries_off` | `index_off + 4 * index_count` |
| `0x1C` | 4 | `entry_count` | |
| `0x20` | 4 | `strings_off` | `entries_off + 8 * entry_count` |
| `0x24` | 4 | `strings_len` | `strings_off + strings_len` = file length |

All five equalities hold in 16,023 of 16,023 files. The reader rejects a file where any fails, and
also a file with `table_count` 0 (that last check is the reader's own; the survey does not state
it). Example, `sm_generator.rbf`:

```text
00: 5242 4620 5630 2e31 | 2800 0000 4200 0000 | 3802 0000 4201 0000 | 4007 0000 4701 0000 | 7811 0000 a105 0000
    "RBF V0.1"            tables 0x28 n=66      index 0x238 n=322     entries 0x740 n=327   strings 0x1178 len 0x5a1
```

### 3.2 Sections

- **Table record** (8 bytes): `u32 child_count`, `u32 first`.
  - count 0: empty table.
  - count 1: `first` is an **entry index directly**.
  - count 2 or more: the children are `entries[index[first + k]]` for `k` in `0..count`.

  The direct-entry case for count 1 is VERIFIED: with this rule a walk from table 0 reaches every
  entry exactly once and every table exactly once in all 16,023 files. Always going through the
  index leaves entries unreachable (`sm_generator`: 294 of 327 reached). Table 0 is the root.
- **Index**: `u32` entry indices.
- **Entry** (8 bytes): the first `u32` holds `type` in its low 16 bits and `key` in its high 16
  bits (bytes `u16 type, u16 key`), then `u32 value`.

| Type | Meaning | Value | Corpus count |
| ---: | --- | --- | ---: |
| 0 | bool | 0 or 1 (only these two seen) | 599,548 |
| 1 | float | `f32` bits | 972,032 |
| 2 | int | `i32`/`u32`. **UCS ids are stored as type 2**; there is no separate localised-string type | 209,103 |
| 3 | string | byte offset into the strings section | 999,077 |
| 4 | table | table index | 968,188 |

No other types occur (VERIFIED). The reader errors on an unknown type, an out-of-range table or
entry, and an out-of-range string.

- **String** at `strings_off + value`: `u32 len`, then `len` bytes, **not** NUL terminated (for
  example `26 00 00 00 "entity_extensions\entity_blueprint_ext"`). Twelve files contain non-ASCII
  bytes; the reader decodes each byte as Latin-1.
- **Child order** inside a table is not sorted by key (392k sorted and 252k unsorted tables).
- **Duplicate keys**: keys repeat inside 27,077 tables (list-like tables such as
  `unit_type_list\type_unit_class`, which has the key twice). Look a key up with a linear scan and
  take the first match; the reader does.

### 3.3 No inheritance to resolve (VERIFIED)

RBFs are compiled flat. `$REF` (type 3) names the **schema template** (`entity_extensions\ui_ext`,
`types\ui_info`, `actions\ability\spawn_squad`), not a parent blueprint. All 197 surveyed
replay blueprints had their `screen_name` inline.

## 4. Field names: `simulation\attrib\fieldnames.flb`

The RBF does not contain key names. The `key` of an entry indexes a global list stored in
`simulation\attrib\fieldnames.flb`. In the analysed install that file exists **only in
`Elite_Campaign_Attrib.sga`** (53,210 bytes); the mount finds it there because the other archives
do not hold it. VERIFIED layout:

| Offset | Field |
| ---: | --- |
| 0 | `u32 n` (2476 in the analysed install) |
| 4 | `n` x `u32` offsets, **relative to byte 4** (the first is `0x26B0`, so the first name is at `0x26B4`) |
| ... | NUL-terminated ASCII names |

Names are unique in the list. Examples: 0 `ability_bag`, 6 `ui`, 36 `$REF`, 43 `ui_info`, 61
`screen_name`, 64 `help_text`, 65 `extra_text`, 746 `ui_ext`, 1324 `squad_ui_ext`, 2061
`upgrade_bag`. These ids are build data. The parser resolves keys by name when it loads the file
and never hard-codes 61.

**Stale list (VERIFIED):** six (file, entry) pairs use key **2476**, one past the end of the list
(`tau_kroot_upgrade_tier2`, `tau_kroot_upgrade_tier3`, `gk_terminator_req_reinforcement_upg`,
`ig_guardsmen_reinforce_adjustment`). A lookup compares keys numerically, so an unknown key is
simply unnamed and never an error.

## 5. Where the screen name lives, per blueprint kind

VERIFIED over the corpus. `screen_name_path()` in `parser_lib/src/blueprints.rs` holds the same
paths. The value is always an entry of type 2 (a UCS id).

| Kind | Path from the RBF root | Files with it / files of the kind |
| --- | --- | --- |
| `ebp` | `ui_ext\screen_name` | 2,179 / 2,591 |
| `sbp` | `squad_ui_ext\screen_name` | 715 / 715 |
| `upgrade` | `upgrade_bag\ui_info\screen_name` | 1,750 / 1,751 |
| `ability` | `ability_bag\ui\ui_info\screen_name` | 2,398 / 2,399 |

Notes from the survey:

- Ebps without `ui_ext` are projectiles and world objects. Some ebps carry other `screen_name`
  fields (`obj_cover_ext\ui_info`, `engineer_ext\...\construction_item\ui_info`,
  `entity_blueprint_ext\screen_name` in 9 files); these are ignored.
- 393 sbps also have `squad_reinforce_ext\ui_info\screen_name`, which is the reinforce button's
  text. It is not used.
- Neighbouring fields: `help_text`, `extra_text`, `brief_text`, `icon_name` (a string). Ebps also
  have `entity_name`, which can differ (`sm_generator`: `entity_name` 9104089 "Power Generator",
  `screen_name` "SM Power Generator").
- **Sentinel:** the id `0` means "no name". A non-zero id that is in no loaded UCS file also means
  "no name".

Traces (entry bytes are `type key value`; offsets are file offsets):

`upgrade\pvp\race_marine\research\tier_2.rbf` (Elite, 1,648 bytes; tables at `0x28` n=27, index at
`0x100` n=68, entries at `0x210` n=75, strings at `0x468` len 520):

```text
table 0  @0x28 : 01000000 00000000   count=1 -> entry 0 directly
entry 0  @0x210: 0400 0d08 01000000  type 4 key 2061 upgrade_bag -> table 1
table 1  @0x30 : 11000000 33000000   count=17 first=51
entry 43 @0x368: 0400 2b00 0e000000  (via index[58] @0x1e8) type 4 key 43 ui_info -> table 14
table 14 @0x98 : 06000000 1f000000   count=6 first=31
entry 45 @0x378: 0200 3d00 ccf28900  (via index[32] @0x180) type 2 key 61 screen_name = 9040588
```

`ebps\pvp\race_marine\buildings\sm_generator.rbf` (Elite, 5,913 bytes):

```text
table 0  @0x28 : 0e000000 34010000   count=14 first=308
entry 22 @0x7f0: 0400 ea02 07000000  (via index[310] @0x710) type 4 key 746 ui_ext -> table 7
table 7  @0x60 : 1a000000 27000000   count=26 first=39
entry 30 @0x830: 0200 3d00 d6ea8a00  (via index[43] @0x2e4) type 2 key 61 screen_name = 9104086
```

## 6. UCS localisation files

Files in `GameAssets\Locale\English\`: `DOW2.ucs` (7.4 MB), `Elite.ucs` (532 KB) and
`Elite_campaign.ucs` (73 KB). `_stock_backup\DOW2.ucs` in the install root is the pre-Elite copy and
differs from the live one in exactly one id (9049220); the parser reads only the live
`GameAssets\Locale\<language>\` files. Only `English` exists in the analysed install; the language
is a parameter of the library (`DEFAULT_LANGUAGE`) and has no CLI flag.

### 6.1 Format (VERIFIED)

- BOM `FF FE`, then UTF-16LE. All text is in the Basic Multilingual Plane, so no surrogate
  handling is needed (the reader rejects invalid UTF-16).
- Records are `<decimal id>\t<text>\r\n`. Split on **CRLF, not LF**: four `DOW2.ucs` values contain
  a bare LF (quotes spanning lines), and splitting on LF would create four bogus lines.
- Split each record on the **first tab** only: 17 `DOW2.ucs` values contain further tabs.
- The last line may lack the CRLF (`Elite_campaign.ucs`).
- 9 `DOW2.ucs` values are empty, and 3,486 have trailing whitespace (for example
  `"Level 2 Stronghold "`), so the text is **trimmed** for display and a blank text counts as no
  name. Ids ascend in `DOW2.ucs` and `Elite.ucs`. No file has duplicate ids.
- Lines without a tab, or with a non-numeric id, are skipped.

### 6.2 Id ranges

| File | Ids | Count |
| --- | --- | ---: |
| `DOW2.ucs` | 1 .. 9,162,924 | 71,720 |
| `Elite.ucs` | 10,000,000 .. 10,100,002 | 2,142 |
| `Elite_campaign.ucs` | 10,000,000 .. 10,100,635 | 604 |

`DOW2.ucs` and `Elite.ucs` are disjoint, so merging them is unambiguous (73,862 strings together).
`UCSBaseIndex = 10000000` in the `[global]` section of `Elite.module` matches the `Elite.ucs` base.
`Elite_campaign.ucs` **collides with `Elite.ucs` on 3 ids**, 2 of them with different text (10100001
"Consecrated Scrolls" against "Perditio"; 10100002 "Intercept" against "All enemies have +50% maximum
health.").

### 6.3 Which files are loaded

HYPOTHESIS (the engine rule was not observed): load `DOW2.ucs` plus `<[global] name>.ucs` (here
`Elite.ucs`). The `DOW2.module` comment says the engine auto-mounts `GameAssets\Locale\<lang>`, and
`dev_pipeline.ini` has `Locale = English`.

The parser loads exactly those two files. `DOW2.ucs` is required; a missing `<name>.ucs`, or a
module with no name, is skipped silently. When two files hold the same id the first one loaded
(`DOW2.ucs`) wins.

`Elite_campaign.ucs` is **not loaded**. It is presumably for the campaign. 156 `screen_name`
references across the corpus resolve only in it, almost all under `xp2\campaign\...`, plus
`ability\pvp\world_objects\pvp_draft\pvp_draft_choose` (10100480 "Choose"). None is referenced by the
surveyed replays.

## 7. Coverage and known limits

Surveyed replays (`temp.rec` and a second replay, both build 4156674300), with the first archive
winning, the paths of section 5 and UCS = `DOW2.ucs` plus `Elite.ucs`:

- `temp.rec`: 166 of 166 **distinct** resolved blueprints have a name (ability 57, upgrade 62, ebp 27,
  sbp 20). They account for 1,014 **occurrences** across `commands` and `actions`; the CLI output
  has 1,014 `blueprint` objects and 1,014 `display_name` fields with empty stderr.
- Both survey replays together: 197 of 197 distinct blueprints (ability 68, upgrade 74, ebp 31,
  sbp 24). The prototype and a Python reference reader gave identical ids for all 197.

Over the whole catalogue (pvp subtree) the lookup does not always produce a name:

| Kind | named | id 0 | id not in UCS | no field |
| --- | ---: | ---: | ---: | ---: |
| ebp, pvp | 389 | 2 | 0 | 32 |
| sbp, pvp | 252 | 7 | 0 | 0 |
| upgrade, pvp | 637 | 109 | 9 | 1 |
| ability, pvp | 770 | 273 | 48 | 1 |

Across all kinds and subtrees 4,297 of 7,456 blueprints get a name. The failures are internal or
hidden blueprints: marker upgrades such as `upgrade\pvp\i_am_retreating`, `ability\pvp\game_mode_packs\ctf_*`,
projectiles such as `csm_blight_grenade`, and Tyranid ids missing from both UCS files (for example
`tyr_synapse`, 9050541). A command that references one of these is plausible but rare; the output
then has no `display_name`.

Observations that can mislead a reader (VERIFIED, game data as shipped):

- `SCMD_ReinforceUnit` (opcode 49) resolves to the **model** ebp, and some models reuse another
  unit's string: ebp `ig_spotter_squad` shows "Guardsman Infantry Squad", `sm_sternguard_veteran`
  shows "Tactical Marine" and `sm_vanguard_veteran_claw` shows "Assault Marine".
- Display names are not unique ("Repair" x3, "Worship" x2, "Sergeant", "Infiltrate",
  "Lascannon"). The blueprint `path` is the identity.
- **Build mismatch:** on a replay from another build the indices already map to wrong blueprints
  (see `replay-format.md`), and the names read as convincingly as correct ones. The parser has no
  build check; `mod_chksum` is in the output so callers can compare builds.
- `fieldnames.flb` lives only in the campaign archive and is one entry short (key 2476, section 4).
  If a future build drops it, the parser reports that display names are unavailable and leaves
  every `display_name` out.
- Storage byte 0 was never observed (section 1.1).
