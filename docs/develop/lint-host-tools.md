# Lint: host tool dependencies and test environments

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
`docs/usage/lint.md` carries the short, user-facing version of the first table. Keep the two in step.
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

## Test environments

The Rules' tools above are the runtime side. The tests need more: the Parity harness builds the
vendored lintian recipes, so it needs a build toolchain and whatever each recipe itself runs. Two
environments matter, and they differ.

### The Parity harness (any host that runs `cargo test`)

| Needs | Why |
| --- | --- |
| `dpkg-dev` (`dpkg-buildpackage`, `dpkg-source`, `dpkg-deb`, `dpkg-parsechangelog`, `dpkg-architecture`, `dpkg-vendor`), `perl`, `make`, `tar`, `fakeroot` | Building the recipes into `.deb`, `.dsc` and Source trees |
| `debhelper` | Nearly every recipe's `debian/rules` calls `dh` |
| `python3` | The `dpkg-deb --nocheck` wrapper the harness writes is a Python script |
| `gpg` (`gnupg`) | Signing the packed orig tarball with the recipe's vendored upstream key |
| `sqv` | dpkg's gpgv backend aborts `dpkg-source -b` on the recipes' fake 27-byte `debian/upstream/signing-key.asc` ("partial read"); with `sqv` installed dpkg uses its Sequoia backend, which does not. Needed by the three `*upstream_signature_missing` source-package tests |
| `lzip`, `lzop` | The `broken-lz` and `broken-lzo` recipes: without them the Rule reports nothing and the Parity test fails |
| `t1utils` (`t1asm`) | The font recipes (`files-adobe-font`) assemble Type 1 fonts while building |

Recipes build with `dpkg-buildpackage -d`, so their `Build-Depends` are **not** checked: a recipe whose
build runs a tool the host lacks fails with that tool's error instead of being skipped. `t1utils`
and `mtools` are examples. Recipes known to be unbuildable on a host are skipped with a reason in
`RECIPES_THE_HOST_CANNOT_BUILD` in `packages/debmagic/tests/common/mod.rs`. Other tests call
`mkfifo` (coreutils) to build fixtures.

### CI (`ubuntu-latest`, currently Ubuntu 26.04)

CI's `cargo test` job installs `debhelper fakeroot lzip lzop t1utils sqv` on top of the runner image
(`.github/workflows/ci.yaml`). Everything in the table above that is not in that list comes from the
image, so a change to the list or to the runner image changes what the tests can check. Ubuntu's
tools are older than the Debian sid host the snapshots and recipes were produced on:

| Difference on CI | Effect | Handling |
| --- | --- | --- |
| Ubuntu's dpkg adds the `noudeb` build profile to every build (`Dpkg::Vendor::Ubuntu`) | debhelper skips udeb packages, so a udeb-only recipe builds nothing (`dpkg-genbuildinfo: binary build with no binary artifacts found`) and a mixed recipe loses its udeb half | The harness sets `DEB_BUILD_PROFILES=!noudeb` when `dpkg-vendor --derives-from ubuntu` succeeds (`dpkg_buildpackage_command`). Debian dpkg does not strip that value and warns about it, hence the condition |
| `sqv` is not installed on the runner image | See the table above | Installed in the workflow |
| `groff` older than 1.25 (Ubuntu ships 1.23) | groff 1.25 ends each warning in its category (`[-w space]`); the LN0270 snapshot and lintian 2.144.0's recipes carry that suffix | The LN0270 unit test returns early (`host_groff_matches_snapshot`). The Parity harness drops expected `groff-message` hints (`host_groff_matches_recipes`) and skips a recipe left with none (`manpage-errors-from-man`) |
| `file` older than 5.47 | No INI detection, and `\x1a` reads as `data`, so the differential tests against `file -b` disagree with our sniffing | `content_type::tests::file_descriptions` returns `None` below 5.47, so `ini_and_ascii_text_agree_with_file_on_generated_fixtures` and `text_kind_agrees_with_file_on_generated_fixtures` skip |
| No JDK, `javahelper`, `dh-r`, `compress`, and a libxcrypt without `setkey` / `encrypt` | Some Parity recipes cannot be built | Listed with a reason in `RECIPES_THE_HOST_CANNOT_BUILD`; recipes whose `Test-Architectures` exclude the host are skipped as well |

The `lzip`/`lzop` unit tests of LN1378 and LN1379 return early when the tool is missing. The Parity
recipes for them do not skip, so a missing tool fails them rather than hiding the Rule.

Skipping means the Rule is **not tested** in those cases, not that it passes. `just lint-parity`
reports what ran on a developer host. The three Ubuntu-only rows above were seen failing in CI; the
`file` version threshold is what the Ubuntu release ships, not something CI prints.

## Adding a host tool

1. Prefer a Rust crate or vendored data over a program. A tool that stays has to justify itself
   here.
2. Resolve it with `external_tool::find` (never rely on a bare name) and return no verdict, not a
   false positive, when it is missing.
3. If a test depends on the tool's version or presence, make the test skip with an `eprintln!` rather
   than fail, and add the row to the tables above.
