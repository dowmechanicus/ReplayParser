# CLAUDE.md

## Model selection

Pick the cheapest model that can do the task reliably. Escalate when the task needs
judgment, interpretation of unknown formats, or when a wrong answer would be written
into docs or code as fact.

### Task-to-model mapping

| Task type | Model / agent | Reasoning | Examples from this repo |
|---|---|---|---|
| Lookup, file moves, locating things | Haiku (`haiku-explore`) | low | "Where does Steam store `temp.rec`?", "copy it here", "locate CLI and command wiring points" |
| Git and PR chores | Haiku (`general-purpose`, model `haiku`) | low | commit and push, open and merge PRs |
| Mechanical docs checks | Haiku (`general-purpose`, model `haiku`) | low | "check docs against session findings" |
| Docs update matching an already-made change | `sonnet-impl-low` | low | fix README command sample tables |
| Bug fix with known root cause plus regression test | `sonnet-impl-low` | low | |
| Small, well-specified parser or module | `sonnet-impl-low` | low | `module.rs` parser |
| Scoped feature touching several files, state or caching | `sonnet-impl-medium` | medium | SGA reader (`sga.rs`), SBP catalogue (`blueprints.rs`), CLI wiring, seat wiring |
| Tests written from acceptance criteria, pure refactors | `sonnet-impl-medium` | medium | |
| Review of a sub-agent's diff against its brief | `sonnet-review-medium` | medium | review of each implementation task |
| Explaining how existing code or docs fit together | Sonnet (main session) | medium | "What does SBP index mean for unit resolution?", "what is opcode 4?" |
| Reverse engineering, format analysis, writing findings as fact | Sonnet, or Opus if novel | high | header checksum meaning, RBF/SGA relationship, seed-to-slot shuffle |
| Planning, orchestration, architecture decisions | Opus | high | planning the SGA reader and splitting it into tasks |
| Final whole-branch review, security or architecture audit | `opus-review-high` | high | review before merge |

### Rules

1. **Haiku never makes claims about unknown formats.** Replay, RBF, SGA and header
   semantics need Sonnet or Opus. A Haiku analysis of the header checksum had to be
   corrected later (commit `bb73b6f`).
2. **Orchestrators coordinate.** If the main session is Opus, delegate routine
   inspection (reading files, running checks, git) to Haiku or Sonnet agents instead
   of running it inline.
3. **Every implementation task gets a Sonnet review agent**, given the brief and the diff.
4. **Docs are not "easy" when they state facts.** A docs change that records a new
   finding is reviewed like analysis. A docs change that only mirrors a code change
   is `sonnet-impl-low`.
5. **Escalate on failure.** If a lower model gets something wrong twice, move one
   tier up rather than retrying.

### When unsure

Ask: could a wrong answer end up in code or docs as fact? Yes means Sonnet or higher.
Is it only finding, moving or committing something? That is Haiku.
