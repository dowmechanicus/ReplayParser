# Blueprint display names: SGA content, RBF and UCS findings

Date: 2026-10-10. Install: `Dawn of War II - Retribution`, `Elite.module`. Replays used:
`temp.rec` (repo root) and `scratchpad/recs/temp2.rec`. Both have mod_chksum 4156674300, which is the
installed build. Parser baseline: `main` @ b0b2f41.

Labels: **VERIFIED** means checked against data, and over the whole corpus where stated.
**HYPOTHESIS** means consistent with the data but not proven. **OPEN** means unknown.

Scratchpad artefacts:
- `tools/sga.py`, `tools/rbf.py`, `tools/ucs.py`: Python reference readers (the `-I` safe ones).
- `tools/sn.py`, `tools/cov.py`, `tools/corpus.py`, `tools/trace.py`: lookup, coverage and the offset tracer
  that produced the tables in §3.4.
- `names/`: a Rust prototype crate (flate2 with the rust backend, plus parser_lib by path) that
  implements the whole pipeline and produced the timings in §6.
- `coverage1.txt` (temp.rec), `coverage_both.txt` (both replays), `rust_lazy.txt` (Rust output).

---

## 1. SGA content extraction

### 1.1 What `sga.rs` does today
It reads the TOC only. `Archive { entries: Vec<Entry{path, data_offset, compressed_len,
uncompressed_len, storage}> }`. It cannot read file content, and it keeps no reader or path after `open`.

**Bug (VERIFIED):** `sga.rs` maps the storage byte as `0 => Stored, 1 => Zlib, other => Unknown`.
On the real archives, **every `.rbf` entry has storage byte 2**. Only `autoblueprints.scar` has 1. Both are
zlib streams (header `78 DA`). So today every RBF is reported as `Storage::Unknown(2)`.
Counts per archive: Elite_Attrib 4532×2 + 1×1, Elite_Campaign_Attrib 709×2 + 1×1,
GameAttrib_orig 10783×2 + 1×1. All 16,027 entries have `compressed_len != uncompressed_len`.
No stored (0) entry exists, so "0 = stored" is a HYPOTHESIS. It is a reasonable fallback that would
need `clen == ulen`.

### 1.2 Layout (version 5.0, all little-endian), VERIFIED on all 3 attrib archives
| offset | size | field | Elite_Attrib.sga example |
|---|---|---|---|
| 0x00 | 8 | magic `_ARCHIVE` | `5f 41 52 43 48 49 56 45` |
| 0x08 | 2+2 | major=5, minor=0 | `05 00 00 00` |
| 0xB0 | 4 | data_start | `c4 00 00 00` (0xC4, the same in all 3) |
| 0xB4 | 4 | toc_offset (absolute) | `62 fa 9f 00` (TOC sits at the **end**, after the data) |
| 0xB8..0xC4 | 12 | unknown (`01 00 00 00`, `00 00 00 00`, 4 bytes that look like a hash) | not needed |

TOC header (24 bytes at toc_offset; offsets are relative to toc_offset): drive (u32 off, u16 n),
folder (u32, u16), file (u32, u16), name (u32, u16). Elite_Attrib: 1 drive, 511 folders,
4533 files, 5044 names. Folder record is 12 bytes and file record is 22 bytes, as `sga.rs` already parses.

File record (22 bytes), example `sm_generator.rbf`, record #1223 at 0xA07C12:
```
fe e7 00 00 | f7 68 3f 00 | d9 09 00 00 | 19 17 00 00 | 58 f3 78 6a | 00 | 02
name_off      data_off      clen=2521     ulen=5913     mtime         flag storage(2=zlib)
```
`data_off` is relative to `data_start`: 0xC4 + 0x3F68F7 = **0x3F69BB**, which starts with `78 da ad 98…`
(the zlib header).

**Per-file preheader (VERIFIED, 16,027/16,027):** 260 bytes sit immediately *before* each file's data
(`data_start + data_off - 260`):
| rel | size | field |
|---|---|---|
| 0 | 256 | basename, NUL-padded, Latin-1 (`sm_generator.rbf\0…`) |
| 256 | 4 | CRC-32 (IEEE, zlib `crc32`) of the **uncompressed** data (`0d b4 d9 1c` = 0x1CD9B40D) |

Files are packed contiguously: gap between consecutive entries = exactly 260 (the preheader). The
first entry has data_off = 260. The last entry ends exactly at toc_offset.

### 1.3 Extraction recipe
`seek(data_start + data_off)`, read `clen` bytes, zlib-inflate (RFC 1950 with header and adler32), and
check `len == ulen`. Optionally check the CRC from the preheader. Python extracted and validated all
16,027 entries (zlib OK, length OK, CRC OK) in 0.2 s.

### 1.4 Dependency
`Cargo.lock` already contains **miniz_oxide 0.8.9** (pulled in via color-eyre → backtrace) and adler2.
Options:
- `miniz_oxide = "0.8"` direct dep: `miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(&buf,
  ulen)`. This adds no new crates to the lock. **Recommended.**
- `flate2 = { version = "1.1", default-features = false, features = ["rust_backend"] }`: adds flate2 and
  crc32fast (both in the local cargo cache; the prototype builds `--offline`). Use it if a `Read` adapter or CRC
  checking is wanted.

---

## 2. Archive priority / override rule

`Elite.module [attrib:common]`: folder.01 `Elite\DataAttrib`, folder.02 `GameAssets\DataAttrib`
(both absent on disk), archive.01 `Elite_Attrib.sga`, archive.02 `Elite_Campaign_Attrib.sga`,
archive.03 `GameAttrib_orig.sga`.

Overlap of `.rbf` paths (archive sets 0=Elite, 1=Campaign, 2=Orig): only-2: 8539, only-0: 2322,
{0,2}: 2201, only-1: 658, {1,2}: 41, {0,1}: 7, {0,1,2}: 2. Of the {0,2} pairs, 2183 differ in content.

**Rule: the lowest `archive.NN` number wins (first in module order).** Elite_Attrib beats
GameAttrib_orig. This is **VERIFIED from replay causality**:
- temp.rec slot 1 casts global ability 960 `pvp\race_marine\globals\sm_spawn_drop_pod` (PCMD_Ability,
  tick 5606). Squad instance 50029 first appears at tick 5711 and from then on gets only
  `sternguard\*` abilities (`sm_toggle_ammo_hellfire`, `…vengeance`, `sm_and_they_shall_know_no_fear`)
  and `SCMD_ReinforceUnit` with ebp `sm_sternguard_veteran`. Slot 1 never builds a sternguard squad
  (its CMD_BuildSquads: tactical, assault, devastator, librarian).
- The **Elite_Attrib** copy of `sm_spawn_drop_pod.rbf` has
  `ability_bag\action_list\start_target_actions\delay\delayed_actions\spawn_squad\squad_name =
  sbps\pvp\race_marine\troops\sm_sternguard_veteran`. The **GameAttrib_orig** copy has no
  `spawn_squad` action at all. So the game ran the Elite copy.
- Supporting UCS evidence: in 28 differing pairs, the Elite copy points to an Elite.ucs id and the orig copy to
  a DOW2.ucs id (for example `ig_commissar_lord` 10001040 "Lord Commissar (Hero)" vs 9159086 "Commissar Lord
  (Hero)", `sm_spawn_drop_pod` 10001497 "Drop Pod Sternguard Veterans" vs 9112951 "Drop Pod"). If orig
  won, those Elite.ucs strings would be dead.

Not discriminated by data (HYPOTHESIS, same rule applied): Elite vs Campaign (9 paths) and Campaign
vs Orig (43 paths). **None of these 52 paths is under `pvp\`**, so they cannot affect multiplayer
replays. Loose folders vs archives: the folders are absent, so OPEN. The suggestion is folders first
(folder.01, folder.02, then archive.01…), matching the module's listing order. If a folder exists, log it.

Result: all 197 blueprints referenced by temp.rec and temp2.rec resolve to the Elite_Attrib copy.

Implementation note: build one `HashMap<lowercase_path, (archive_idx, entry_idx)>` and keep the
**first** insertion while iterating archives in module order. Archive paths are already all lowercase.

---

## 3. RBF layout (`RBF V0.1`), all little-endian

### 3.1 Header (40 bytes), VERIFIED on all 16,023 `.rbf` entries in the 3 archives
| off | size | field |
|---|---|---|
| 0x00 | 8 | magic `RBF V0.1` |
| 0x08 | 4 | tables_off (always 0x28) |
| 0x0C | 4 | table_count |
| 0x10 | 4 | index_off (= tables_off + 8·table_count) |
| 0x14 | 4 | index_count |
| 0x18 | 4 | entries_off (= index_off + 4·index_count) |
| 0x1C | 4 | entry_count |
| 0x20 | 4 | strings_off (= entries_off + 8·entry_count) |
| 0x24 | 4 | strings_len (strings_off + strings_len = file length) |

All five equalities hold in 16,023/16,023 files. Example `sm_generator.rbf`:
```
00: 5242 4620 5630 2e31 | 2800 0000 4200 0000 | 3802 0000 4201 0000 | 4007 0000 4701 0000 | 7811 0000 a105 0000
    "RBF V0.1"            tables 0x28 n=66      index 0x238 n=322     entries 0x740 n=327   strings 0x1178 len 0x5a1
```

### 3.2 Sections
- **Table record** (8 bytes): `u32 child_count, u32 first`.
  - count 0: empty table.
  - count 1: `first` is an **entry index directly**.
  - count ≥ 2: children are `entries[index[first + k]]` for k in 0..count.

  The direct-entry case for count 1 is VERIFIED. Under this rule, a walk from table 0 reaches every entry exactly once and
  every table exactly once in all 16,023 files. The "always via index" alternative leaves entries
  unreachable (sm_generator: 294/327).
- **Table 0 is the root.**
- **Index table**: `u32` entry indices.
- **Entry** (8 bytes): `u16 type, u16 key, u32 value`.

  | type | meaning | value | corpus count |
  |---|---|---|---|
  | 0 | bool | 0/1 (only these two seen) | 599,548 |
  | 1 | float | f32 bits | 972,032 |
  | 2 | int | i32/u32. **UCS ids are stored as type 2**; there is no separate locstring type | 209,103 |
  | 3 | string | byte offset into the strings section | 999,077 |
  | 4 | table | table index | 968,188 |

  No other types occur.
- **String** at `strings_off + value`: `u32 len` followed by `len` bytes, **not** NUL-terminated
  (`26 00 00 00 "entity_extensions\entity_blueprint_ext"`). Twelve files contain non-ASCII bytes, so decode
  as Latin-1 or lossily.
- Child order inside a table is **not** sorted by key (392k sorted vs 252k unsorted tables). **Keys may
  repeat** (27,077 tables, for list-like tables such as `unit_type_list\type_unit_class` ×2), so look a key up
  with a linear scan and take the first match.

### 3.3 Keys: global dictionary `simulation\attrib\fieldnames.flb`
Key names are not in the RBF. `key` indexes a global field-name list stored in
`simulation\attrib\fieldnames.flb`. That file exists **only in Elite_Campaign_Attrib.sga** (53,210
bytes). Layout:
| off | field |
|---|---|
| 0 | u32 n (= 2476) |
| 4 | n × u32 offsets, **relative to byte 4** (first = 0x26B0, giving a string at 0x26B4) |
| … | NUL-terminated ASCII names |

Name lookups used here: 0 `ability_bag`, 6 `ui`, 36 `$REF`, 43 `ui_info`, **61 `screen_name`**, 64
`help_text`, 65 `extra_text`, 746 `ui_ext`, 1324 `squad_ui_ext`, 2061 `upgrade_bag`. Names are unique
in the flb.

The flb is slightly stale. Six (file, entry) hits use key **2476**, one past the end of the list
(`tau_kroot_upgrade_tier2/3`, `gk_terminator_req_reinforcement_upg`,
`ig_guardsmen_reinforce_adjustment`). The reader must treat unknown keys as unnamed, not as an error.
Resolve key ids by name from the flb at load time instead of hard-coding 61. Key ids are build data.

### 3.4 Where the screen name lives (per kind), VERIFIED over the corpus
| kind | path from root | files having it / kind total |
|---|---|---|
| ebps | `ui_ext\screen_name` | 2179/2591 (others: projectiles, world objects; some also have `obj_cover_ext\ui_info\…`, `engineer_ext\…\construction_item\ui_info\…`, `entity_blueprint_ext\screen_name` (9). Ignore these) |
| sbps | `squad_ui_ext\screen_name` | 715/715 (393 also have `squad_reinforce_ext\ui_info\screen_name` = the reinforce button. Do not use it) |
| upgrade | `upgrade_bag\ui_info\screen_name` | 1750/1751 |
| ability | `ability_bag\ui\ui_info\screen_name` | 2398/2399 |

The value is always type 2 (int, a UCS id). `0` is the "no name" sentinel. A non-zero id that is
missing from the UCS files also means "no name". Neighbouring fields: `help_text`, `extra_text`,
`brief_text`, `icon_name` (string). Ebps also have `entity_name` (sm_generator: 9104089 "Power
Generator" vs screen_name "SM Power Generator").

**Inheritance:** none needs to be resolved. RBFs are compiled flat. `$REF` (type 3) names the
*schema template* (`entity_extensions\ui_ext`, `types\ui_info`, `actions\ability\spawn_squad`), not a
parent blueprint. Every one of the 197 replay blueprints had its screen_name inline.

Hex trace (from `tools/trace.py`): entry bytes are `type key value`.

`upgrade\pvp\race_marine\research\tier_2.rbf` (Elite, 1648 B; tables@0x28 n=27, index@0x100 n=68,
entries@0x210 n=75, strings@0x468 len 520):
```
table 0 @0x28 : 01000000 00000000      count=1 → entry 0 directly
entry 0 @0x210: 0400 0d08 01000000     type 4 key 2061 upgrade_bag → table 1
table 1 @0x30 : 11000000 33000000      count=17 first=51
entry 43 @0x368: 0400 2b00 0e000000    (via index[58]@0x1e8) type 4 key 43 ui_info → table 14
table 14 @0x98: 06000000 1f000000      count=6 first=31
entry 45 @0x378: 0200 3d00 ccf28900    (via index[32]@0x180) type 2 key 61 screen_name = 9040588
```
`ebps\pvp\race_marine\buildings\sm_generator.rbf` (Elite, 5913 B):
```
table 0 @0x28 : 0e000000 34010000      count=14 first=308
entry 22 @0x7f0: 0400 ea02 07000000    (via index[310]@0x710) table key 746 ui_ext → table 7
table 7 @0x60 : 1a000000 27000000      count=26 first=39
entry 30 @0x830: 0200 3d00 d6ea8a00    (via index[43]@0x2e4) int key 61 screen_name = 9104086
```
`ability\…\ig_stormtrooper_squad_infiltrate.rbf` (Elite, 4531 B):
```
table 0 @0x28 : 01000000 00000000  → entry 0 @0x4c0: 0400 0000 01000000  ability_bag → t1
table 1 @0x30 : 23000000 b3000000  → entry 11 @0x518: 0400 0600 03000000 ui → t3       (index[184])
table 3 @0x40 : 12000000 31000000  → entry 16 @0x540: 0400 2b00 05000000 ui_info → t5  (index[51])
table 5 @0x50 : 07000000 07000000  → entry 18 @0x550: 0200 3d00 d5ba8b00 screen_name = 9157333 (index[8])
```
`sbps\pvp\race_marine\troops\sm_tactical_marine.rbf` (Elite, 4947 B):
```
table 0 @0x28 : 0e000000 b1000000  → entry 34 @0x5d4: 0400 2c05 08000000 squad_ui_ext → t8 (index[179])
table 8 @0x68 : 13000000 31000000  → entry 39 @0x5fc: 0200 3d00 60ae8900 screen_name = 9023072 (index[53])
```

---

## 4. UCS

Files in `GameAssets\Locale\English\`: `DOW2.ucs` (7.4 MB), `Elite.ucs` (532 KB),
`Elite_campaign.ucs` (73 KB). `_stock_backup\DOW2.ucs` in the install root is the pre-Elite copy and differs
from the live one in exactly one id (9049220). **Use the live `GameAssets\Locale\…` file.**

Format (VERIFIED):
- BOM `FF FE`, then UTF-16LE (all BMP, so no surrogate handling needed).
- Records are separated by **CRLF**: `<decimal id>\t<text>\r\n`. **Split on `\r\n`, not `\n`.** Four DOW2
  values contain bare LF (quotes spanning lines). Splitting on LF produces four bogus lines with no tab.
- Split on the **first** tab only. 17 DOW2 values contain further tabs.
- The last line may lack CRLF (`Elite_campaign.ucs`).
- 9 DOW2 values are empty. 3,486 DOW2 values have trailing whitespace (for example `"Level 2 Stronghold "`,
  `"Grenade Barrage "`), so **trim** for display. Ids are ascending in DOW2 and Elite.
- No duplicate ids within any file.

Id ranges: DOW2.ucs 1..9,162,924 (71,720 ids); Elite.ucs 10,000,000..10,100,002 (2,142 ids);
Elite_campaign.ucs 10,000,000..10,100,635 (604 ids). DOW2 and Elite are **disjoint**, so merging them into one map
is unambiguous. `UCSBaseIndex = 10000000` in Elite.module `[global]` matches the Elite.ucs base.
Elite_campaign **collides with Elite.ucs on 3 ids**, 2 with different text (10100001
"Consecrated Scrolls" vs "Perditio", 10100002 "Intercept" vs "All enemies have +50% maximum health.").

Which files to load (HYPOTHESIS: the engine rule is not observed directly): `DOW2.ucs` + `<[global] Name>.ucs`
(`Elite.ucs`). The DOW2.module comment says the engine auto-mounts `GameAssets\Locale\<lang>`.
`dev_pipeline.ini` has `Locale = English`. Elite_campaign.ucs is presumably for the campaign. 156
screen_name references across the corpus resolve **only** in Elite_campaign.ucs, almost all
`xp2\campaign\…`, plus `ability\pvp\world_objects\pvp_draft\pvp_draft_choose` (10100480 "Choose").
None is referenced by the test replays. Recommendation: load DOW2 + Elite, and optionally Elite_campaign as
**fill-only** (never overriding an existing id).

Language: only `English` exists in this install (no other `Locale\*` dirs; the sound sections name
other languages but the folders are absent). Make the language a parameter (default `English`, the folder
`GameAssets\Locale\<lang>`).

---

## 5. Coverage

**Replays:** with the prototype (winner = first archive, per-kind path §3.4, UCS = DOW2 ∪ Elite,
trimmed, empty or 0 → none):
- temp.rec: **166/166** unique resolved blueprints named (ability 57, upgrade 62, ebp 27, sbp 20; 1,014
  occurrences across `commands` and `actions`).
- temp.rec ∪ temp2.rec: **197/197** (ability 68, upgrade 74, ebp 31, sbp 24). No failures.
- The Rust prototype and the Python reference give identical ids for all 197.

**Whole catalogue**, for an idea of what failures look like (pvp subtree / all):
| kind | ok | zero (0) | missing id | no field |
|---|---|---|---|---|
| ebp pvp | 389 | 2 | 0 | 32 |
| sbp pvp | 252 | 7 | 0 | 0 |
| upgrade pvp | 637 | 109 | 9 | 1 |
| ability pvp | 770 | 273 | 48 | 1 |
| all kinds, all subtrees | 4297 of 7456 | | | |

The failure classes are internal or hidden blueprints:
- zero: `upgrade\pvp\i_am_retreating`, `*_is_active` marker upgrades, `ability\pvp\game_mode_packs\ctf_*`,
  `sbps\…\csm_chaos_nuke_core_aura*`, `ig_banewolf`.
- no field: projectiles such as `ebps\pvp\race_chaos\projectiles\csm_blight_grenade`, `gk_thunderhawk`,
  `tau_markerlight_revealer_pvp`.
- missing id: ids absent from both UCS files, mostly Tyranid, e.g. `tyr_synapse` 9050541.

A command referencing one of these is plausible but rare, so the fallback is the existing `name` (basename).

Observations a reader of the output may trip over (VERIFIED, game data as shipped):
- Reinforce (op 49) resolves to the **model** ebp, and some models reuse another unit's string:
  ebp `ig_spotter_squad` → "Guardsman Infantry Squad", ebp `sm_sternguard_veteran` → "Tactical Marine",
  ebp `sm_vanguard_veteran_claw` → "Assault Marine".
- Display names are not unique: "Repair" ×3, "Worship" ×2, "Sergeant", "Infiltrate", "Lascannon". Keep `path` as the
  identity.

---

## 6. Cost

Measured with the Rust prototype (`names/`, release build, this machine). The baseline is the current
debug `cargo run` on temp.rec: 1.37 s wall, 107 MB max RSS.
| step | time |
|---|---|
| open 3 archive TOCs + build winner map | 4 ms |
| load DOW2.ucs + Elite.ucs (73,862 strings) | 15 ms |
| **eager**: inflate + parse + lookup all 7,456 winning blueprint RBFs (43 MB inflated, streamed) | 114 ms |
| **lazy**: only the 197 referenced (temp + temp2) | 3 ms |
| peak RSS (either mode, dominated by UCS map + TOCs) | ~35 MB |

Python takes 2.2 s for all 16,023 RBFs. A debug Rust build will be several times slower than release. Lazy
stays in the low milliseconds anyway.

**Recommendation: lazy.** After command resolution, collect the unique `(kind, index)` set
(~150–200 per game), then read only those RBFs. Eager costs about 0.1 s release and more in debug, for nothing. UCS
must be loaded whole (one file read, ~15 ms). It can be skipped when no blueprint resolved.

---

## 7. Recommended architecture

```
ModuleFile (+ [global] Name, UCSBaseIndex)
   │
   ▼
AttribMount  (new, e.g. attrib.rs)   one pass over [attrib:common] archives in module order
   ├─ archives: Vec<(PathBuf, sga::Archive)>
   ├─ index: HashMap<lowercase path, (archive_idx, entry_idx)>   first wins
   ├─ paths() → feeds BlueprintCatalogues::from_paths (replaces its own archive loop)
   └─ read(path) → io::Result<Vec<u8>>                           via sga::Archive::read_entry
   │
   ├─ fieldnames.flb ──► rbf::FieldNames (name ↔ u16 key)
   ▼
rbf.rs  Rbf::parse(&[u8]) -> Rbf; Rbf::get_path(&[u16]) -> Option<Value>
        Value = Bool | Float | Int(u32) | Str(String) | Table(u32)
   │
ucs.rs  UcsTable::load(path) ; merge(DOW2, <Name>.ucs) ; get(id) -> Option<&str>
   │
blueprints.rs  BlueprintKind::screen_name_path() -> &[&str]
               ResolvedBlueprint { …, display_name: Option<String> }  (skip_serializing_if None)
   ▲
parser CLI: after resolving commands/actions → names = DisplayNames::lookup(mount, ucs, unique set)
```

Module-level changes:
1. **sga.rs**: map storage `1 | 2 => Zlib`. Keep `0 => Stored` (require `clen == ulen`). Add
   `read_entry<R: Read+Seek>(r, &Entry) -> io::Result<Vec<u8>>` that bounds-checks
   `data_offset + clen <= len`, inflates with a limit of `ulen`, and checks the length (optionally the CRC32 from the
   preheader at `data_offset - 4`). Either store the archive `PathBuf` in `Archive`, or let the mount own
   a `File` per archive. Tests: synthetic archive with a zlib entry + stored entry + bad zlib + truncated.
2. **attrib mount**: shared by the catalogues and the name lookup, so each SGA TOC is parsed once
   (today `BlueprintCatalogues::from_module` opens them and discards the entries).
3. **rbf.rs** (~150 lines): header validation (the five equalities in §3.1 as errors), the table walk with the
   count==1 rule, bounds checks on every index, key, offset and string length, and no recursion needed for a path
   lookup. Unknown key ids are allowed.
4. **ucs.rs** (~60 lines): BOM check, UTF-16LE decode, split on `\r\n`, first tab, parse id,
   trim on read. Make the language a parameter.
5. **Output**: add `display_name` to `ResolvedBlueprint`. Optionally add `screen_name_id` for debugging. Failure to load
   names (no flb, no UCS) should warn once and leave `display_name` absent. It must not fail the parse.

New deps: `miniz_oxide = "0.8"` (already in the lockfile) or flate2 with the rust_backend feature.

### Test pins (real install, gate on `DOW2_INSTALL`, `#[ignore]` like the existing ones)
| kind | index | path (relative to kind dir, no .rbf) | screen_name id | text (trimmed) | note |
|---|---:|---|---:|---|---|
| ebp | 661 | pvp\race_marine\buildings\sm_generator | 9104086 | SM Power Generator | |
| ebp | 425 | pvp\race_chaos\buildings\csm_generator | 9131802 | Chaos Power Generator | |
| ebp | 607 | pvp\race_imperial_guard\heroes\ig_commissar_lord | 10001040 | Lord Commissar (Hero) | Elite override (orig 9159086) |
| sbp | 252 | pvp\race_marine\troops\sm_tactical_marine | 9023072 | Tactical Marine Squad | |
| sbp | 129 | pvp\race_chaos\vehicles\csm_landraider | 10000078 | Land Raider Phobos | Elite.ucs id |
| sbp | 147 | pvp\race_eldar\troops\eld_guardian | 10000037 | Guardian Squad | Elite override (orig 9049207 "Guardian") |
| upgrade | 475 | pvp\race_marine\research\tier_2 | 9040588 | Level 2 Stronghold | raw UCS has trailing space |
| upgrade | 50 | pvp\race_chaos\research\tier_2 | 9111652 | Level 2 Chaos Temple | |
| upgrade | 382 | pvp\race_imperial_guard\troops\ig_leman_russ_vanquisher | 10000534 | Vanquisher | |
| ability | 906 | pvp\race_imperial_guard\stormtrooper_squad\ig_stormtrooper_squad_infiltrate | 9157333 | Infiltrate | |
| ability | 962 | pvp\race_marine\globals\sm_spawn_terminators | 9112967 | Deep Strike Terminator Squad | global |
| ability | 960 | pvp\race_marine\globals\sm_spawn_drop_pod | 10001497 | Drop Pod Sternguard Veterans | override pin (orig 9112951 "Drop Pod") |
| ability | 777 | pvp\race_imperial_guard\devils\ig_grenade_launch_triple | 10000603 | Ol' Unreliable | override (orig 9152930 "Ol' Reliable") |
| ebp | 441 | pvp\race_chaos\projectiles\csm_blight_grenade | — (no `ui_ext`) | None | negative |
| upgrade | 49 | pvp\i_am_retreating | 0 | None | negative (sentinel) |
| ability | 1326 | pvp\race_tyranid\tyr_synapse | 9050541 | None | negative (id not in UCS) |

Pin offsets for a unit test of the RBF reader on an extracted fixture: see §3.4 (tier_2 screen_name
entry at file offset 0x378, bytes `02 00 3d 00 cc f2 89 00`). Other pins: SGA `sm_generator.rbf` in
Elite_Attrib: data at 0x3F69BB, clen 2521, ulen 5913, storage 2, preheader CRC 0x1CD9B40D. UCS:
`DOW2.ucs` count 71,720 (CRLF split), `Elite.ucs` 2,142.

A small real RBF (tier_2 is 1,648 bytes) could be checked in as a test fixture, but it is game data. Building a synthetic RBF
in the test, like the `sga.rs` tests do, avoids shipping game files.

---

## 8. Open risks
- **Build mismatch** (unchanged from catalogue-findings §0): on old replays the indices map to wrong
  blueprints, and the names will be confidently wrong. Display names make this more convincing. Consider gating
  on `mod_chksum`.
- **fieldnames.flb** lives only in the campaign archive and is one entry short (key 2476). If a future
  build drops it, name lookup must degrade gracefully. Keys must be resolved by name, not hard-coded.
- **Archive order** is VERIFIED only for archive.01 vs archive.03. Archive.02's rank and the loose folders are
  HYPOTHESES (no pvp impact today).
- **UCS file selection** (DOW2 + `<Name>.ucs`) is inferred, not observed in the engine. Elite_campaign
  collides on 3 ids.
- **Storage 0** (uncompressed) was never observed.
- Duplicate or shared display strings (§5) mean the name is not an identity. Keep `path`.
