# Lint: host tool dependencies and known CI limitations

`debmagic lint` runs in-process on the host, but some Rules shell out to programs the wheel and the
binary do **not** bundle. Every such dependency is a limitation twice over:

- **Portability.** `uvx debmagic lint` on a non-Debian distribution (or a minimal container) has
  whatever set of tools that system happens to ship, in whatever version. A missing tool usually
  makes a Rule report nothing, silently; a different version can change what it reports.
- **CI.** CI runs on `ubuntu-latest`, whose tools are older than the Debian sid host the snapshots
  and the vendored lintian recipes were produced on. Tests that assume the newer behaviour have to
  skip there.

This page is the ledger of both. **When you add a Rule or a test that runs a host program, add it
here.** The long-term fix is to bundle or replace the tool (the way `file(1)` was replaced by the
goblin-based classification); until
then, an entry should say what happens when the tool is absent.

```{note}
`docs/usage/lint.md` still says lint has "no external tools". That is no longer true; this page is
the accurate list.
```

## Tools the Rules run

Most are found on `PATH` through `lint::external_tool::find` and run in a sandbox directory with a
cleared environment (`external_tool.rs`). The behaviour without the tool is what the code does
today, not a promise.

| Tool (package) | Used by | Without the tool |
| --- | --- | --- |
| `dpkg-source` (`dpkg-dev`) | Extracting the patched tree of a `.dsc` Subject (`source_package.rs`) | Lint of that Subject fails with "dpkg-source is unavailable" |
| `tar` | Extracting source tarballs (`source_package.rs`); must be GNU tar for `--touch` / `--no-same-owner` | Lint of that Subject fails with "tar is unavailable" |
| `man`, `lexgrog` (`man-db`), and the `groff` it drives | `groff-message` LN0270 (`man --warnings`); the whatis Rules LN0274, LN0275, LN0279 (`lexgrog`) | No hints from those Rules |
| `file` (`file`) | `national-encoding` LN0018 (`file --raw` descriptions) | No hints. The description text differs between `file` releases |
| `/bin/dash`, `/bin/bash` | `*-shell-script-fails-syntax-check` LN1425, LN1426, LN1742 (`-n`), at these absolute paths | No verdict for that interpreter |
| `bzip2` | `broken-bz2` LN1377 | No hints |
| `lzip` or `clzip` | `broken-lz` LN1378 | No hints |
| `lzop` | `broken-lzo` LN1379 | No hints |
| `appstreamcli` (`appstream`) | `appstream-metadata-validation-failed` LN1467 | No hints |
| `msgfmt` (`gettext`) | `invalid-po-file` LN0383 | No hints |
| `/usr/share/intltool-debian/intltool-update` (`intltool-debian`) and `msgcmp` (`gettext`) | The pot comparison behind `invalid-potfiles-in` LN1326, `newer-debconf-templates` LN1327, `no-complete-debconf-translation` LN1531 | The comparison is `Unknown`, so no hints |

Notes:

- `man` keeps its own seccomp sandbox on. We do not set `MAN_DISABLE_SECCOMP`; where `man` cannot
  run under the host's seccomp policy the Rule sees no output.
- Everything else lint needs is a Rust crate linked at build time (`flate2`, `xz2`, `bzip2`, `zstd`,
  `goblin`, `ar`, `tar`) or vendored data, so it is not a runtime host dependency.

## Known CI limitations

CI's `cargo test` job runs on `ubuntu-latest` and installs only `debhelper` and `fakeroot` on top.
These are the tests that cannot check what they were written to check there, and what we do about
each. The first three are observed failures; the version numbers of the `file` row are what the
Ubuntu release ships, not something CI prints.

| Limitation on CI | Effect | Workaround today |
| --- | --- | --- |
| `groff` older than 1.25 (Ubuntu ships 1.23) | groff 1.25 ends each warning in its category (`[-w space]`); the LN0270 snapshot and lintian 2.144.0's recipes carry that suffix | The LN0270 unit test returns early (`host_groff_matches_snapshot`). The Parity harness drops expected `groff-message` hints (`host_groff_matches_recipes`) and skips a recipe left with none (`manpage-errors-from-man`) |
| `file` older than 5.47 | No INI detection, and `\x1a` reads as `data`, so the differential tests against `file -b` disagree with our sniffing | `content_type::tests::file_descriptions` returns `None` below 5.47, so `ini_and_ascii_text_agree_with_file_on_generated_fixtures` and `text_kind_agrees_with_file_on_generated_fixtures` skip |
| `lzip` / `clzip` and `lzop` not installed | LN1378 and LN1379 report nothing, so their "must fail" cases pass | Their `binary` tests return early when the tool is missing |
| No JDK, `javahelper`, `dh-r`, `compress`, and a libxcrypt without `setkey` / `encrypt` | Some Parity recipes cannot be built | Listed with a reason in `RECIPES_THE_HOST_CANNOT_BUILD` in `packages/debmagic/tests/common/mod.rs`; recipes whose `Test-Architectures` exclude the host are skipped as well |

Skipping means the Rule is **not tested on CI** in those cases, not that it passes. They run on a
developer host that has the tools (`just lint-parity` reports what ran). Other tests call `mkfifo`
(coreutils) to build fixtures and the Parity harness calls `dpkg-source`, `dpkg-buildpackage`,
`dpkg-deb`, `dpkg-parsechangelog`, `dpkg-architecture`, `perl`, `make` and `tar`; those are
development-host requirements, not runtime ones.

## Adding a host tool

1. Prefer a Rust crate or vendored data over a program. A tool that stays has to justify itself
   here.
2. Resolve it with `external_tool::find` (never rely on a bare name) and return no verdict, not a
   false positive, when it is missing.
3. If a test depends on the tool's version or presence, make the test skip with an `eprintln!` rather
   than fail, and add the row to the tables above.
