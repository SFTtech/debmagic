# Ecosystem run: real archive packages, one work packet per Rule

The ecosystem suite (`scripts/lint_ecosystem.py`, `just lint-ecosystem`) lints **real Debian archive
packages** and diffs `debmagic lint` against the lintian results Debian's infrastructure already
reports for that exact version (UDD's `lintian_results`, the data behind the lintian link on
tracker.debian.org). The vendored test-suite stays the CI oracle (ADR 0012); this is the
real-world measurement that finds what recipes never isolate.

Use it in two modes:

- **Spot check** — a handful of packages, for a Rule you are already working on.
- **Bulk run** — a large package set, to produce **work packets**: one Markdown file per Rule that
  disagrees, ready to hand to an agent.

## Requirements

- Network access to `deb.debian.org` and `udd-mirror.debian.net`.
- `psql` (`postgresql-client`) for the oracle; `lintian` only when measuring performance.
- A release build of `debmagic` (the default profile); debug timings are not representative.

## Bulk run

```shell
# 25 key packages plus samba, correctness only: fastest useful run
just lint-ecosystem --sample 25 --perf off

# Every key source package in sid: the full run (about 23k Subjects, tens of GB of downloads)
just lint-ecosystem --all --perf off

# A reproducible sample from the whole archive, not just key packages
just lint-ecosystem --sample 200 --pool archive --seed 7 --perf off

# Exactly the packages you care about
just lint-ecosystem --packages samba hello dgit --perf off

# The pinned 400-package set (samba plus a sample of 400 key packages, seed 20261123)
just lint-ecosystem --package-list tests/ecosystem/packages-400-seed20261123.txt --perf off
```

## Reproducible sets

A `--sample N --seed S` run is reproducible only while UDD's tables do not change: new uploads change
versions, key packages come and go, and oversized packages are dropped by their current size. So a
set to rerun later is pinned in a file, one `source version` pair per line:

```shell
just lint-ecosystem --sample 400 --seed 20261123 --list-only \
    --write-package-list tests/ecosystem/packages-400-seed20261123.txt
just lint-ecosystem --package-list tests/ecosystem/packages-400-seed20261123.txt --perf off
```

`--list-only` stops after the selection (no downloads, no lint). The file's header records the
selection, the UDD release and date, and the oracle's lintian version. A pinned version that sid no
longer lists is reported and skipped, so regenerate the file when a pinned version has been replaced. A bare name, without a version, takes every version
sid lists for it: the Sources index can hold two (as for `gcc-16`), and both are linted.
The committed `tests/ecosystem/packages-400-seed20261123.txt` pins 401 packages: `samba` plus the
400 sampled ones, selected on 2026-11-23.

The lint options default to `--select LN --show-overrides`, so overridden hints come back as `O:`
lines for the comparison. Set `--lint-arg` (repeatable) to replace them.

`--perf off` is the right default for a correctness run: the perf comparison re-lints every Subject
with lintian and can triple the wall time. Downloads and oracle snapshots are cached under
`target/lint-ecosystem/`, so a rerun (or a packet's single-subject check) does not download again;
`--refresh-oracle` re-queries UDD when you want a newer lintian oracle.

Reads over thousands of packages are long, so the run reports where it is: one stamped line per
stage on stderr, and a tqdm bar for downloads (files fetched or already cached) and one for linting
(Subjects done, with the current Subject as the postfix). Bars are drawn only when stderr is a TTY;
`--progress on` forces them, `--progress off` gives plain lines, and `-v` adds a line per Subject.

```text
[19:02:15] start: lint ecosystem run into target/lint-ecosystem (progress: auto)
[19:02:16] select: querying a sample of 20 (key pool, seed 3) in sid via UDD
[19:02:16] select: 21 source package(s) selected
[19:02:16] oracle: 3 of 21 snapshot(s) cached, querying 18 via UDD
[19:02:16] downloads: 402 file(s) queued with 4 worker(s)
[19:02:16] lint: linting 121 Subject(s) with --select LN
downloads:  64%|██████▎   | 258/402 [00:41<00:23, 6.1file/s, samba]
lint:  40%|████      | 48/121 [01:12<01:50, 1.6subject/s, binary samba:amd64 2:4.25.1+dfsg-1]
[19:04:30] lint: done: 121 Subject(s) compared, 0 failure(s), debmagic 159s, 206 disagreement(s) across 35 Tag(s)
[19:04:31] report: wrote target/lint-ecosystem/report.md and report.json
[19:04:31] done: finished in 135s
```

Downloads start in the background as soon as the package set is known, so the download bar keeps
moving while the lint bar waits on the first package. A Subject whose download failed or whose
`debmagic lint` crashed is written above the bars and counted in the run's failure total.

## Outputs

All under `--work-dir` (default `target/lint-ecosystem/`):

| Path | Content |
| --- | --- |
| `report.md` | Aggregate: recall/precision, per-Tag table, a few human-readable examples, uncovered Tags, performance |
| `report.json` | The same numbers plus **every** disagreement, machine-readable |
| `rules/index.md` | Work packets ranked by how many Subjects they affect |
| `rules/<CODE>_<tag>.md` | One packet per disagreeing Rule |

### What a packet contains

Each packet is self-contained, so an agent needs no other file to start:

- the Rule source path and Code;
- how many Subjects disagree, split by *missed* / *extra* / *context* / *severity*;
- for every affected Subject — not a truncated sample — the artifact path and the exact hint lines
  on each side;
- a copy-paste repro command per Subject and a warm-cache rerun command for its source packages;
- a closure check: native tests for the new behaviour, `just lint-parity` unchanged, and the Subject
  no longer disagreeing.

```text
target/lint-ecosystem/rules/index.md
    | tag                                | code   | Rule                                    | Subjects | missed | extra |
    | source-contains-autogenerated-...  | LN1620 | packages/.../ln1620_..._gperf_data.rs   |       12 |     12 |     0 |
    | postrm-does-not-purge-debconf      | LN0883 | packages/.../ln0883_postrm_...rs        |        9 |      0 |     9 |
```

### Handing packets to agents

Give an agent **one packet, not the whole report** — a packet is already scoped to a Rule, a bounded
set of Subjects and a verification command. A batch run splits naturally along `rules/index.md`:

```shell
# one agent per packet, in its own worktree
scripts/agent_prep.sh <base-commit>
# ...then hand over target/lint-ecosystem/rules/LN1620_<tag>.md
```

Each agent works the same loop: reproduce with the packet's `debmagic lint --select` command, read
the Rule and the lintian Check (see [Import a lintian Rule](import-lintian-rule.md)), fix it, add
native tests, run `just lint-parity`, then re-run the packet's `just lint-ecosystem --packages …`
line and confirm the Subject dropped out.

## Reading the comparison

- **missed** — lintian reports the Tag, debmagic does not (false negative). This includes a hint
  debmagic hid with an override that lintian does not apply.
- **extra** — debmagic reports it, lintian does not (false positive).
- **unapplied** — an extra where lintian hides the Tag on this Subject by an override and debmagic
  shows it as a normal hint: the override pass did not apply the override (ADR 0057).
- **overridden (both)** — both sides hide the Tag by an override: the override pass agrees.
- **context** — both report it, but the hint contexts (`[path:line]`, ...) differ as multisets.

`mismatched-override` and `unused-override` (LN1779, LN1780) come from the override pass itself and
are judged like any other covered Tag once they are in the Rule catalogue.

Only Tags with an LN Rule are judged; other oracle Tags are listed as **uncovered** and ranked by
weight, which is the input for choosing the next Rule to port.

A `context` disagreement is often a harness or formatting difference rather than a Rule bug: check
whether lintian's own spelling of the pointer differs before changing Rule logic. A **failed**
Subject (download, timeout, crash) is listed separately and is never counted as a miss or an extra —
fix it first, or the packet is silent about it.

## Caveats

- The oracle is the lintian version reported in UDD, not necessarily the installed one; tag renames
  are folded onto the current names, but a newer or older local lintian can disagree for reasons
  that are not debmagic's.
- Real packages exercise Rules on inputs the recipes do not; a disagreement can be a genuine Rule
  bug or a lintian bug. The packet shows both sides, so judge each one.
- Duplicate hints and ordering are not compared; only the multiset of hint contexts and the
  severity per Subject.