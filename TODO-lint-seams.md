# Open lint seams

A *seam* is a Rule-facing inspectable, parser, or harness capability that a Tag needs and we do not have yet. This file is the ledger: every open seam with the Tags it blocks, plus a short record of what landed. It is updated whenever an agent reports a new seam, a seam changes status, or a seam lands. The per-inspectable narrative stays in [TODO-lint.md](TODO-lint.md).

**Last updated**: 2026-11-23, milo tip `f929f1c8`, 1459 LN Rules (LN1767–LN1778 added by the no-oracle batch), Parity suite 1818 tests green. Every Tag that has an oracle recipe and needs no new seam is implemented; what remains with recipes is listed below. The no-oracle Tags LN1767, LN1768, LN1770–LN1778 have native tests only.
**Measure**: `just lint-parity` (`--candidates N` for ranked next Rules).

**Status words**: *In progress* (an agent is on it), *Approved* (the user agreed to the design, not started), *Proposed* (written up, waiting for a go), *Needs decision* (a design or policy question for the user), *Deferred* (decided not to do now), *Landed* (in milo).

## Needs a decision

| Question | Why | Recommendation |
| --- | --- | --- |
| Run Group Rules that report on an installable for every Group, including one with no `.dsc` (a lone `.deb`, or `.deb` plus `.changes`)? Today a Group Rule runs only when the Group has a `.dsc` (ADR 0055) | Lintian runs these per-binary checks with optional access to the group's source. `cannot-check-whether-usr-share-doc-symlink-points-to-foreign-package` (LN1521) is exactly the no-source case; `gir-missing-typelib-dependency` (LN1499) and `usr-share-doc-symlink-to-foreign-package` (LN1506) do not run on a lone `.deb` either | Yes: make `ctx.dsc()` optional for Group Rules that report on installables, amend ADR 0055, and let the harness build a lone-`.deb` Group for the recipe `copyright-file-foreign-package`. Existing Group Rules return early without a `.dsc` |
| ~~Override application pass~~ | Decided 2026-11-23: build it. Design in ADR 0057 (`docs/adr/0057-override-application-pass.md`, draft): `--show-overrides` / `--no-override`, hidden by default, new `OverrideRule` trait, `mismatched-override` default-selected, unused overrides of info and pedantic Tags are reported as lintian does | Implement in the task order of the ADR |

## Open seams, in progress

None running. The seams below are proposed and each blocks exactly the Tags named.

## Open seams, proposed (not started)

| Seam | Blocked Tags | Notes |
| --- | --- | --- |
| Harness: honour the recipe's `Build-Command` / build options (`buildinfo=+path`) on upload-group builds | `file-references-package-build-path` (LN1561; GroupRule written and reverted: the `.buildinfo` has no `Build-Path`) | May change other Group recipes; needs its own check |
| Harness: recipes with `--include-dir` data overrides (`eval/lintian-include-dir`, e.g. `experimental=10` in `data/debhelper/compat-level`) | `package-uses-experimental-debhelper-compat-version` (LN1681 reserved) | Rule is trivial once the override can be modelled (a test-only threshold) |
| Raw or signed `.changes` (`GroupContext::changes_path()` or `Changes::is_signed()`) | `unreleased-changelog-distribution` (LN1664 reserved) | Small accessor |
| Diffstat of the Debian diff (`SourceContext::diffstat()` from `<name>_<version>.diff.gz` via sibling files) | `direct-changes-in-diff-but-no-patch-system`, `patch-system-but-direct-changes-in-diff` (LN1706 reserved) | Medium: a unified-diff file-list parser matching diffstat |
| Source extraction messages (tar warnings of the patched and orig trees, `ctx.unpack_messages()`) | `unpack-message-for-source` (LN1711 reserved), `unpack-message-for-orig` (no recipe) | Small to medium |
| Orig-file mtimes (host trees from filesystem metadata, or read orig tarball headers) | `orig-tarball-contains-ancient-file` (LN1461) | `Entry::mtime()` exists for Binary packages |
| Group Rules without a `.dsc` (decision above) | `cannot-check-whether-usr-share-doc-symlink-points-to-foreign-package` (LN1521) | |
| Override application pass (decision above) | `mismatched-override`, `unused-override` | |
| Group behaviour: a bad argument skips and continues (lintian) instead of aborting the run | none (behaviour only) | From the group seam, ADR 0054; recommendation: follow lintian |
| Group Parity for `source-*` recipes (the product is a `.dsc` only) | group hints of those recipes are not covered | From the group seam |
| Policy-release table refresh to the 2.144.0 pin | accuracy of `out-of-date-standards-version` | The table is from lintian 2.139.0; `lint/data/mod.rs` has a stale comment |
| Static-library per-member ELF errors for `elf-error`; `elf-warning` readelf WARNINGS | `elf-warning` (no vendored recipe, so no Parity) | |

## Not seams: Tags with no usable oracle

- `debhelper-compat-not-a-number` (LN1305): lintian 2.144.0 never emits it; the recipe cannot be matched natively. Dropped.
- `elf-warning` (LN1460): no vendored recipe.
- `debian-rules-missing-recommended-target`: no lintian Check emits it (the Tag's own text says there are no recommended targets). No Rule.
- `manual-page-in-udeb` (LN1776) and `debian-rules-uses-deprecated-makefile` (LN1768) are *dead* in lintian 2.144.0 (unreachable hint, empty `deprecated-makefiles` table). Both Rules are implemented to report nothing; LN1776 needs a decision whether to report the stated intent instead.
- LN1770 `patch-modifying-debian-files`: one hint per patch name (lintian's greedy regex may merge them); only unified diffs are read; a symlinked `debian/patches` is not followed.

## Landed in milo

| Seam | Landed with |
| --- | --- |
| Multi-artifact runs: Group, `.changes` and `.buildinfo` Subjects, `ChangesRule` / `BuildinfoRule` / `GroupRule`, testers, `new-rule --kind`, Group Parity (ADR 0054, 0055) | LN1440, LN1441; LN0374 for Changes and Buildinfo |
| Group Rules may report on an installable (`GroupContext::installable_diagnostic`); Group Parity compares binary and udeb hints | LN1499–LN1508 |
| `debian/control` field positions, raw `resembles` tables | LN1312–LN1315 |
| XML documents (`roxmltree`) and AppStream metadata | LN1320, LN1462–LN1466 |
| Spelling module and printable-strings view (`fancy-regex`) | LN1301, LN1310, LN1311, LN1423, LN1468, LN1408, LN1542, LN1543 |
| `debian-binary` bytes and any-ar-archive reader, `Entry::mtime()`, `BinaryPackageContext::installed_changelog()` | LN1304, LN1382, LN1418 |
| ELF facts (section names, machine, class, endianness, readelf-style errors) and shared `ar` parser | LN1290–LN1293, LN1416 |
| MD5 digests (`digest.rs`, `FileTree::md5sum`) and content-type sniffing (`content_type.rs`) | LN1375, LN1376 |
| Upstream metadata YAML (`ctx.upstream_metadata()`, `yaml-rust2`) | LN1337, LN1338, LN1339–LN1342, LN1344 |
| Pinned, regenerable iso-codes tables (`scripts/vendor_iso_codes.py`, iso-codes 4.20.1) | LN1385, LN1386, LN1589 |
| Parity harness: build cache (cold about 70–110 s, warm about 10 s), non-native orig packing, importer keeps continuation lines, pointer-form normalisation for three cruft Tags (and the two Python obsolete Tags), `FileTree::read_prefix` | LN1388–LN1390, LN1571, LN1572 |
| Parity harness round 2: `post-build` and `pre-orig` hooks, `Upload-Type: source` (`-S`) builds, Group Rules reporting `changes` hints (`GroupContext::changes_diagnostic`), FIFO hint drop, orig signing for upstream-signature recipes | LN1486, LN1510, LN1516, LN1491, LN1586, LN1564 |
| External tool runner (`external_tool.rs`: sandboxed host tool with timeout; shared `script_syntax`, po-debconf pot comparison) | LN1425, LN1426, LN1326, LN1327, LN1531, LN1467, LN1377–LN1379 |
| Type 1 font parser (`type1.rs`, eexec decryption) | LN1393, LN1394 |
| Sibling files of a `.dsc` (names from the `Files` / `Checksums-*` fields, bytes from the directory) and tar unpack messages | LN1434–LN1437, LN1590, LN1433 |
| ELF symbol sections, symbol-table flag, ELF facts of `ar` members | LN1401, LN1574, LN1534 |
| `.changes` checksums, `GroupContext::direct_reliants`, native OpenPGP packet reader (`openpgp.rs`), `signing_key_files()` | LN1484, LN1540, LN1535 |

## Cleanups noticed along the way (not seams)

- `fields_version::source_format` / `is_native` for a Source tree with no `.dsc`: lintian's `Format.pm` treats `1.0` without a diff as native whatever the revision, but the Parity recipes (`general-false-positives/basic-non-native-1.0`, `tracking/generic-dh-make-2005/2008`, and the watch/upstream-metadata cases) are built from a real `.dsc` and expect non-native hints for `1.0-1` trees with and without `debian/source/format`. Following the Perl regressed 8 Parity cases and 15 unit fixtures and gained none, so the current `!contains('-')` rule stays (decided 2026-11-23 on that evidence). Revisit only if those recipes are re-derived from a `.dsc`-less build.
- The recipe `checks/cruft/cruft-general-upstream` expects `source-contains-*` for a non-native package that lintian 2.144.0 cannot emit: its expected hints for the 13 Tags are dropped (`drop_expected_hints_for_non_native_upstream`, documented in `docs/develop/import-lintian-rule.md`).
- LN1764/LN1765 name a symlinked signing key by its target path; lintian names the link.
- LN1720 `manual-page-for-system-command` cannot see manual pages from direct dependencies; LN1723 lacks the emacsen/elpa and CGI exemptions; LN1729 does not follow symlinked units.
- LN0005 test fix: the run test patches `Caml1999X` out of the copied test binary so it no longer depends on rodata layout (`6ed8f0c3`).
- LN0928 `license-problem-md5sum-non-free-file` has its own MD5 and table parsing; it could use `FileTree::md5sum` and `digest::hex_digest`.
- The Vcs Rules other than LN1372/LN1373 (LN0135, LN1240–1243, LN0023, LN0797–LN0800) probably miss the `XS-Vcs-*` alias on a Source tree (a shared helper exists in `rules/lintian/vcs.rs`).
- `scripts/new_lint_rule.py` sometimes adds a bogus `pub(crate) use lintian::<Struct>;` line for root-level source rules; agents remove it by hand.
- `unpack-message-for-deb-control` (LN1591) has no vendored recipe; a one-line Rule on `Tarball::Control` once a recipe exists.
- `non-ascii-path-failure/success` recipes have a vendored `post-build` hook (moves the product into a Latin-1-named directory), not tried yet.
- A tool that is missing or times out gives no verdict in the external tool runner (lintian would emit a hint); the sandbox does not isolate the network.
- LN0383 `invalid-po-file` still calls msgfmt directly instead of the tool runner.
- Pytest `tests/integration/test_packages.py::test_build_package[htop]` fails when the whole `tests/` directory runs (not part of `just test`); not investigated.
- Doc comment of `binary-with-bad-dynamic-table` (LN1162) still says `elf-error` is not implemented (it is LN1292).
- The three `source_missing` Rules (LN1388–LN1390) sit in a top-level file like LN1387; moving them with LN0004, LN1086 and LN1387 into a `files_source_missing/` module is optional.
- Upstream-signature recipes not yet re-imported with their `pre-build` hooks: repeated-signature, explicit-armor-header, doubly-armored-signature, control-file-upstream-signature-missing, and a dfsg variant (the siblings seam re-imports what it needs).

## Decisions already made

- Branch: lint work integrates onto `milo/debmagic-lint-subcommand`.
- Pointer form `[path]` is the oracle truth for `source-is-missing`, `source-contains-prebuilt-javascript-object` and `source-contains-prebuilt-python-object`; the harness normalises the bare form. The same normalisation extends to the two Python Tags (LN1571, LN1572).
- iso-codes: vendored from an explicit pinned version (4.20.1) with a regeneration script.
- `fancy-regex`, `roxmltree` and `yaml-rust2` are accepted dependencies.
- Overrides stay parsed, not applied, for now.
