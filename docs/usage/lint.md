# Linting packages

Quick reference for `debmagic lint`, a built-in port of [lintian](https://lintian.debian.org)'s checks.

## TL;DR

- Entry point: `debmagic lint [PATH...]` — a source tree (default, the current directory), or any number of artifacts: `.deb`/`.udeb`/`.ddeb`, `.dsc`, `.changes`, `.buildinfo`
- Runs in-process on the host: no container, no `lintian`, no external tools
- Exits non-zero when a Diagnostic of a `--fail-on` severity is found (default: `error` only)

```shell
debmagic lint                         # source tree in the current directory
debmagic lint ../foo_1.0-1_amd64.deb  # a binary package
debmagic lint ../foo_1.0-1.dsc        # a source package
debmagic lint ../foo_1.0-1_amd64.changes   # everything the .changes names, as one group
debmagic lint ../foo_1.0-1.dsc ../foo-doc_1.0-1_all.deb   # artifacts of one source package are one group
```

## What a run lints

A source tree is linted on its own.
Artifacts are put into *groups*, as lintian does:

- every path becomes a group, and a `.changes`, `.dsc` or `.buildinfo` pulls in the `.deb`, `.udeb`, `.ddeb`, `.dsc` and `.buildinfo` files it names from its own directory (a named file that is missing is an error);
- groups of the same source package and version (`<source>_<version>`) merge, so naming the `.changes` and one of its `.deb` files again changes nothing;
- a group has at most one `.changes`, one `.dsc` and one `.buildinfo`, and any number of installables.

Each artifact is inspected by the Rules for its kind, and a few Rules look at the whole group (for example `intra-source-package-circular-dependency`).
Diagnostics name their package and kind as lintian does: `foo (changes)`, `foo (source)`, `foo-doc (binary)`, `foo (buildinfo)`.
A source tree cannot be mixed with artifacts, and nothing is discovered for you: with no path lint reads the current directory, never `../*.changes`
([ADR 0054](../adr/0054-lint-run-takes-artifacts-and-lints-groups.md), [ADR 0055](../adr/0055-group-rule-and-changes-buildinfo-subjects.md)).

## Available options

| Option | Description |
| --- | --- |
| `--select` | Run only these Rules, by Code (`LN0006`), Tag (`package-contains-unsafe-symlink`) or Code prefix. Replaces the default set |
| `--ignore` | Remove Rules from the current set, same selectors |
| `--fail-on` | Severities that make lint exit non-zero: `error`, `warning`, `info`, `pedantic` |
| `-j`, `--threads` | Worker threads (default: available parallelism) |
| `--timings` | Print how long each Rule took |

`debmagic lint --help` lists the rest.

## Lintian Tags we skip

Rules are ported from lintian and checked against lintian's own test suite, so a Tag that is not here is either not ported *yet* or skipped on purpose.
These are the deliberate skips.

| Tags | Why |
| --- | --- |
| Classification Tags (`C:`, for example `control-tarball-compression-format`) | They are metadata, not findings. They get no Code and cannot be selected ([ADR 0019](../adr/0019-classification-tags-are-not-rules.md)) |
| Tags whose Check runs an external program: shell syntax (`*-shell-script-fails-syntax-check`), AppStream validation (`appstream-metadata-*`), integrity of `.bz2`/`.lz`/`.lzo`/`.zip` files (`broken-*`), po-debconf and PostScript font Tags | Lint has no host-tool dependencies and no environment ([ADR 0013](../adr/0013-lint-has-no-environment.md); `file(1)` was dropped for the same reason, [ADR 0041](../adr/0041-file-type-is-goblin-classification.md)) |
| `elf-error`, `elf-warning` | Their extra is `readelf`'s message text; the ELF parser's errors read differently, so a port could never match |
| `mismatched-override` | Lintian override files are parsed, not applied ([ADR 0051](../adr/0051-lintian-overrides-are-parsed-not-applied.md)); `--select`/`--ignore` are not overrides ([ADR 0009](../adr/0009-selection-is-not-override.md)) |

### Not yet implemented

Gaps that are missing inspectables, not decisions:

- Most `.changes`, `.buildinfo` and group Tags: `.changes`, `.buildinfo` and whole groups are inspected now, but only a few of their Tags are ported (`bad-section-in-changes-file`, `intra-source-package-circular-dependency`, and the Changes and Buildinfo side of `debian-revision-not-well-formed`)
- `possible-bashism-in-maintainer-script` needs a port of `checkbashisms`
- `malformed-deb-archive`: a `.deb` that cannot be opened does not become a Subject
- Members of static libraries (`.a`) are not inspected, so the `elf-by-member` variants of the hardening and obsolete-crypt Tags do not fire
- Jars using zip64 or compression other than stored and deflate are reported as unreadable

For the porting workflow and the per-Rule status, see [Importing a lintian Rule](../develop/import-lintian-rule.md).
