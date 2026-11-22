# Linting packages

Quick reference for `debmagic lint`, a built-in port of [lintian](https://lintian.debian.org)'s checks.

## TL;DR

- Entry point: `debmagic lint [SUBJECT]` — one Subject per run: a source tree (default, the current directory), a `.deb`/`.udeb`/`.ddeb`, or a `.dsc`
- Runs in-process on the host: no container, no `lintian`, no external tools
- Exits non-zero when a Diagnostic of a `--fail-on` severity is found (default: `error` only)

```shell
debmagic lint                         # source tree in the current directory
debmagic lint ../foo_1.0-1_amd64.deb  # a binary package
debmagic lint ../foo_1.0-1.dsc        # a source package
```

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
| `.changes` Tags (`bad-distribution-in-changes-file`, `checksum-mismatch-in-changes-file`, `bad-urgency-in-changes-file`, ...) | `.changes` is not a Subject kind yet ([ADR 0016](../adr/0016-lint-subject-kinds.md)) |
| Tags that compare several packages of one source (`binaries-have-file-conflict`, `intra-source-package-circular-dependency`, `dependency-is-not-multi-archified`, `doc-base-file-references-missing-file`) | Lintian checks a whole group of artifacts at once; a lint run has exactly one Subject ([ADR 0017](../adr/0017-one-subject-per-lint-run.md)) |
| Tags whose Check runs an external program: shell syntax (`*-shell-script-fails-syntax-check`), AppStream validation (`appstream-metadata-*`), integrity of `.bz2`/`.lz`/`.lzo`/`.zip` files (`broken-*`), po-debconf and PostScript font Tags | Lint has no host-tool dependencies and no environment ([ADR 0013](../adr/0013-lint-has-no-environment.md); `file(1)` was dropped for the same reason, [ADR 0041](../adr/0041-file-type-is-goblin-classification.md)) |
| `elf-error`, `elf-warning` | Their extra is `readelf`'s message text; the ELF parser's errors read differently, so a port could never match |
| `mismatched-override` | Lintian override files are parsed, not applied ([ADR 0051](../adr/0051-lintian-overrides-are-parsed-not-applied.md)); `--select`/`--ignore` are not overrides ([ADR 0009](../adr/0009-selection-is-not-override.md)) |

### Not yet implemented

Gaps that are missing inspectables, not decisions:

- `possible-bashism-in-maintainer-script` needs a port of `checkbashisms`
- `malformed-deb-archive`: a `.deb` that cannot be opened does not become a Subject
- Members of static libraries (`.a`) are not inspected, so the `elf-by-member` variants of the hardening and obsolete-crypt Tags do not fire
- Jars using zip64 or compression other than stored and deflate are reported as unreadable

For the porting workflow and the per-Rule status, see [Importing a lintian Rule](../develop/import-lintian-rule.md).
