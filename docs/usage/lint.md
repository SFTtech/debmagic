# Linting packages

Quick reference for `debmagic lint`, a built-in port of [lintian](https://lintian.debian.org)'s checks.

## TL;DR

- Entry point: `debmagic lint [PATH...]` — a source tree (default, the current directory), or any number of artifacts: `.deb`/`.udeb`/`.ddeb`, `.dsc`, `.changes`, `.buildinfo`
- Runs in-process on the host: no container, no `lintian`. A few Rules call host programs when they are installed, see [Host tools](#host-tools)
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
A source tree cannot be mixed with artifacts, and nothing is discovered for you: with no path lint reads the current directory, never `../*.changes`.

## Available options

| Option | Description |
| --- | --- |
| `--select` | Run only these Rules, by Code (`LN0006`), Tag (`package-contains-unsafe-symlink`) or Code prefix. Replaces the default set |
| `--ignore` | Remove Rules from the current set, same selectors |
| `--fail-on` | Severities that make lint exit non-zero: `error`, `warning`, `info`, `pedantic` |
| `-j`, `--threads` | Worker threads (default: available parallelism) |
| `--timings` | Print how long each Rule took |
| `--show-overrides` | Also print the Diagnostics a Lintian override file suppressed, as `O:` lines (see [Lintian override files](#lintian-override-files)). Conflicts with `--no-override` |
| `--no-override` | Do not apply Lintian override files: nothing is suppressed and `unused-override` and `mismatched-override` have nothing to report |

`debmagic lint --help` lists the rest.

## Lintian override files

A processable's Lintian override file is applied after the Rules that ran on it: each source tree or source package reads `debian/source/lintian-overrides` (or `debian/source.lintian-overrides`), and each binary package reads `usr/share/lintian/overrides/<package>` (or its `.gz`) from the `.deb` or `.udeb`. The file is read only when the processable has a Diagnostic or an override Rule is selected.

An override suppresses a Diagnostic when its Tag matches and its context pattern, if it has one, matches the Diagnostic's message followed by `[pointer]` for a Diagnostic with a path. A pattern uses `*` (any run of characters) and `?` (one character). A `#` comment above a line is its justification.

- A suppressed Diagnostic is not printed and does not count towards `--fail-on` or the summary. A run whose only Diagnostics are suppressed prints `All checks passed!`.
- `--show-overrides` prints each suppressed Diagnostic as an `O:` line, after an `N:` line with its justification when there is one.
- Unless `--show-overrides` or `--no-override` is given, a line after the summary counts what was suppressed and what is unused, for example `N: 3 hints overridden; 1 unused override`.
- `unused-override` (`LN1779`, info) reports an override that matched no Diagnostic, and `mismatched-override` (`LN1780`, warning) one whose Tag produced a Diagnostic that the pattern did not match. Both only cover Rules that ran: an override of a Tag that is not selected is not reported, so select the Tag (`--select cute-field`) to check its overrides.
- An override of a Tag we [skip on purpose](#lintian-tags-we-skip) is never reported, because no Rule for it ever runs. This differs from lintian for Classification Tags: lintian runs their Checks (it only hides the `C:` output) and reports a stale override of one as `unused-override`, and debmagic stays silent. For example, `wpasupplicant` overrides the Classification Tag `possible-gpl-code-linked-with-openssl`: lintian reports that override as `unused-override` when its Check emits nothing, and debmagic does not report it at all.
- An override of a durable Tag is ignored, because lintian declares these Tags `Show-Always` and they cannot be suppressed: `alien-tag`, `duplicate-override-context`, `invalid-override-restriction`, `malformed-override`, `mismatched-override`, `renamed-tag` and `unused-override`. The run then prints `N: Some overrides were ignored.`
- An override with an `[architecture]` list applies only to a processable of one of those architectures, as in lintian.

Overrides are checked per processable: a source override never suppresses a binary package's Diagnostic, and the other way round.

(host-tools)=
## Host tools

Archive formats, ELF and Type 1 fonts, file classification and everything else lint needs are built in. These host programs are the exception.
Install them for the matching Rules to run.

| Program (Debian package) | Needed for |
| --- | --- |
| `dpkg-source` (`dpkg-dev`), GNU `tar` | Linting a `.dsc`, which is unpacked as `dpkg-source -x` does |
| `man`, `lexgrog` (`man-db`) and `groff` | `groff-message` and the manual-page `whatis` Tags |
| `file` | `national-encoding` |
| `bzip2`, `lzip` (or `clzip`), `lzop` | `broken-bz2`, `broken-lz`, `broken-lzo` |
| `appstreamcli` (`appstream`) | `appstream-metadata-validation-failed` |
| `msgfmt`, `msgcmp` (`gettext`) and `intltool-update` (`intltool-debian`) | `invalid-po-file`, `invalid-potfiles-in`, `newer-debconf-templates`, `no-complete-debconf-translation` |
| `dash`, `bash` at `/bin/dash` and `/bin/bash` | `*-shell-script-fails-syntax-check` |

Without a program, lint does not fail: the Rules that need it report nothing, so a clean run on a bare system proves less than on a full one.
The exception is a `.dsc`, whose lint fails when `dpkg-source` or `tar` is missing.
Some Tags also differ between versions of a program, for example `groff-message` needs groff 1.25 or newer to match lintian's text.
The developer-facing [list of host tools](../develop/lint-host-tools.md) says what each program does and how the tests cope without it.

## Lintian Tags we skip

Rules are ported from lintian and checked against lintian's own test suite, so a Tag that is not here is either not ported *yet* or skipped on purpose.
These are the deliberate skips.

| Tags | Why |
| --- | --- |
| Classification Tags (`C:`, for example `control-tarball-compression-format`) | They are metadata, not findings. They get no Code and cannot be selected |
| `elf-error`, `elf-warning` | Their extra is `readelf`'s message text; the ELF parser's errors read differently, so a port could never match |

### Not yet implemented

Every other lintian Tag has a Rule. What remains are gaps inside some Rules, from inputs that are not inspected yet:

- `malformed-deb-archive` reports a `.deb` whose ar members are wrong; a file that cannot be read as an ar archive at all fails the run instead
- Members of static libraries (`.a`) are not inspected, so the `elf-by-member` variants of the hardening and obsolete-crypt Tags do not fire
- Jars using zip64 or compression other than stored and deflate are reported as unreadable

For the porting workflow and the per-Rule status, see [Importing a lintian Rule](../develop/import-lintian-rule.md).
