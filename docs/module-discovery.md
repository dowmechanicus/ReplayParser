# Module discovery: finding `Elite.module` without `--module`

The parser resolves blueprint indices into names against a `.module` file (see the README
section "Resolving blueprint names (`--module`)"). This note records how the parser finds that
file when no `--module` argument is given. The behaviour is implemented in
[`parser/src/discover.rs`](../parser/src/discover.rs), wired in
[`parser/src/main.rs`](../parser/src/main.rs).

## 1. Precedence, VERIFIED by test

Candidates are tried in this order. The first accepted candidate wins.

| # | Source | Check | Evidence |
| ---: | --- | --- | --- |
| 0 | `--module` argument | Used as given; discovery does not run | VERIFIED (unchanged behaviour) |
| 1 | `DOW2_INSTALL` | `Elite.module` directly in that directory, case-insensitive | VERIFIED by test when set; falls through to 2 when unset or empty |
| 2 | Steam apps by id | `steamlocate::SteamDir::locate()`, then `find_app` per id below; `Library::resolve_app_dir` + `Elite.module` | VERIFIED live (see §2) |
| 3 | Library filename scan | `steamapps\common\<dir>\Elite.module` for every `<dir>` in every library, case-insensitive, top level only | VERIFIED by test |

App ids, ordered by preference: **56400** `Warhammer 40,000: Dawn of War II: Retribution`
(what Elite Mod extends), then **15620** the Anniversary Edition (Classic), which bundles
Retribution. Both are VERIFIED against the Steam appdetails API (2026-10).

## 2. Live observation, VERIFIED

On a machine with Steam and Retribution installed, `cargo run -p parser -- temp.rec` without
`--module` located

```text
.../steamapps/common/Dawn of War II - Retribution/Elite.module
```

and produced the documented catalogue counts (ebp 2591, sbp 715, upgrade 1751, ability 2399)
and display names.

## 3. Selection rules, VERIFIED by test

- An app-id candidate (rank = position in the id list) beats any scan candidate.
- Paths are deduplicated case-insensitively, keeping the lowest rank; the app-id pass runs
  first, so a path found by both passes keeps the app-id rank.
- With several remaining candidates: the first by rank (library order breaks ties) is used,
  and stderr lists every candidate in a `warning:`.
- With none: the parse continues without names and stderr reports the searched libraries and
  the `--module` hint. Discovery failure never fails the parse.
- A found module behaves exactly like a `--module` argument: its path lands in
  `ReplayInfo.blueprints.module`, and the install dir is the module's parent.

## 4. Scope and open points

- `steamlocate` reads `appmanifest_*.acf`, so it only sees *installed* apps; owned-but-not-installed
  games produce no candidate. VERIFIED (library behaviour of the crate).
- The scan is top-level only: `common\<app>\Sub\Elite.module` is not found. HYPOTHESIS that no
  real install nests the module; the `--module` argument covers it.
- The Anniversary Edition (15620) install-dir layout was not observed. HYPOTHESIS; its
  appmanifest `installdir` is read, not guessed, and the scan is the fallback.
- Non-Steam copies (retail) are supported through `DOW2_INSTALL` or `--module` only.
