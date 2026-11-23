# Open lint seams

A *seam* is a Rule-facing inspectable, parser, or harness capability that a Tag needs and we do not have yet. This file is the ledger: every open seam with the Tags it blocks, plus a short record of what landed. It is updated whenever an agent reports a new seam, a seam changes status, or a seam lands. The per-inspectable narrative stays in [TODO-lint.md](TODO-lint.md).

**Last updated**: 2026-11-23 morning, milo tip `42888952`, 1303 LN Rules (1097 at the start of the batch work), Parity suite 1656 tests green.
**Measure**: `just lint-parity` (`--candidates N` for ranked next Rules).

**Status words**: *In progress* (an agent is on it), *Approved* (the user agreed to the design, not started), *Proposed* (written up, waiting for a go), *Needs decision* (a design or policy question for the user), *Deferred* (decided not to do now), *Landed* (in milo).

## Needs a decision

| Question | Why | Recommendation |
| --- | --- | --- |
| Run Group Rules that report on an installable for every Group, including one with no `.dsc` (a lone `.deb`, or `.deb` plus `.changes`)? Today a Group Rule runs only when the Group has a `.dsc` (ADR 0055) | Lintian runs these per-binary checks with optional access to the group's source. `cannot-check-whether-usr-share-doc-symlink-points-to-foreign-package` (LN1521) is exactly the no-source case; `gir-missing-typelib-dependency` (LN1499) and `usr-share-doc-symlink-to-foreign-package` (LN1506) do not run on a lone `.deb` either | Yes: make `ctx.dsc()` optional for Group Rules that report on installables, amend ADR 0055, and let the harness build a lone-`.deb` Group for the recipe `copyright-file-foreign-package`. Existing Group Rules return early without a `.dsc` |
| Override application pass (suppress Diagnostics with parsed overrides)? | Blocks `mismatched-override` and `unused-override`; ADR 0051 says overrides are parsed, not applied | Defer until wanted |

## Open seams, in progress

| Seam | Branch | Blocked Tags (reserved Codes) |
| --- | --- | --- |
| Parity harness round 2: `post-build` hooks; `Upload-Type: source` builds (`dpkg-buildpackage -S`); Group Rules attributed to the `changes` processable type; FIFO hints dropped from the expected side; buildinfo `ARCHITECTURE` rewrite; `build-spec/pre-orig` | `lint-harness2` | `checksum-count-mismatch-in-changes-file` (LN1486), `changelog-distribution-does-not-match-changes-file` (LN1516; needs the post-build hook), `source-only-upload-to-non-free-without-autobuild` (LN1510; needs `-S`), `inconsistent-maintainer` (LN1491), `runtime-test-file-is-not-a-regular-file` (LN1586), `upstream-file-without-utf8-name` (LN1564), buildinfo case of `package-has-long-file-name` |
| External tool runner (materialise a File-tree subset, run a host tool with a timeout), then compressed-stream integrity | `lint-seam-tools` | `maintainer-shell-script-fails-syntax-check` (LN1425), `shell-script-fails-syntax-check` (LN1426), `invalid-potfiles-in` (LN1326), `newer-debconf-templates` (LN1327), `no-complete-debconf-translation` (LN1531), `appstream-metadata-validation-failed` (LN1467; recipe fixture reports "Passed" on appstreamcli 1.2.3, so a host skip is likely), `broken-bz2` (LN1377), `broken-lz` (LN1378), `broken-lzo` (LN1379) |
| Sibling files of a `.dsc` (names plus lazy bytes of its directory), then tar unpack messages | `lint-seam-siblings` | `concatenated-upstream-signatures` (LN1434), `doubly-armored-upstream-signature` (LN1435), `explicitly-armored-upstream-signature` (LN1436), `orig-tarball-missing-upstream-signature` (LN1437), `spurious-fields-in-upstream-signature` (LN1590), `unpack-message-for-deb-data` (LN1433), `unpack-message-for-deb-control` (LN1591) |
| Type 1 font parser (eexec decryption, no external tool) | `lint-seam-type1` | `license-problem-font-adobe-copyrighted-fragment` (LN1393), `license-problem-font-adobe-copyrighted-fragment-no-credit` (LN1394) |
| ELF follow-ups: symbol-table fact, ELF facts of `ar` members, per-symbol section names | `lint-seam-elf2` | `ocaml-custom-executable` (LN1401), `static-link-time-optimization` (LN1574), `no-symbols-control-file` (LN1534) |
| Follow-ups on landed seams: `.changes` checksums with the digest helper, `GroupContext::direct_reliants`, native OpenPGP packet reader | `lint-seam-followups` | `checksum-mismatch-in-changes-file` (LN1484), `spare-manual-page` (LN1540), `public-upstream-key-unusable` (LN1535) |
| Batch follow-up: Group Rule for the buildinfo `Build-Path`; pointer-form normalisation extended to the two Python Tags | `lint-n100-misc-3` | `file-references-package-build-path` (LN1561), `debian-pycompat-is-obsolete` (LN1571), `debian-pyversions-is-obsolete` (LN1572) |

## Open seams, proposed (not started)

| Seam | Blocked Tags | Notes |
| --- | --- | --- |
| Orig-file mtimes (host trees from filesystem metadata, or read orig tarball headers) | `orig-tarball-contains-ancient-file` (LN1461) | `Entry::mtime()` exists for Binary packages |
| Group behaviour: a bad argument skips and continues (lintian) instead of aborting the run | none (behaviour only) | From the group seam, ADR 0054; recommendation: follow lintian |
| Group Parity for `source-*` recipes (the product is a `.dsc` only) | group hints of those recipes are not covered | From the group seam |
| Policy-release table refresh to the 2.144.0 pin | accuracy of `out-of-date-standards-version` | The table is from lintian 2.139.0; `lint/data/mod.rs` has a stale comment |
| Static-library per-member ELF errors for `elf-error`; `elf-warning` readelf WARNINGS | `elf-warning` (no vendored recipe, so no Parity) | |

## Not seams: Tags with no usable oracle

- `debhelper-compat-not-a-number` (LN1305): lintian 2.144.0 never emits it; the recipe cannot be matched natively. Dropped.
- `elf-warning` (LN1460): no vendored recipe.

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
| Parity harness: build cache (cold about 70–110 s, warm about 10 s), non-native orig packing, importer keeps continuation lines, pointer-form normalisation for three cruft Tags, `FileTree::read_prefix` | LN1388–LN1390 |

## Cleanups noticed along the way (not seams)

- LN0928 `license-problem-md5sum-non-free-file` has its own MD5 and table parsing; it could use `FileTree::md5sum` and `digest::hex_digest`.
- The Vcs Rules other than LN1372/LN1373 (LN0135, LN1240–1243, LN0023, LN0797–LN0800) probably miss the `XS-Vcs-*` alias on a Source tree (a shared helper exists in `rules/lintian/vcs.rs`).
- `scripts/new_lint_rule.py` sometimes adds a bogus `pub(crate) use lintian::<Struct>;` line for root-level source rules; agents remove it by hand.
- Doc comment of `binary-with-bad-dynamic-table` (LN1162) still says `elf-error` is not implemented (it is LN1292).
- The three `source_missing` Rules (LN1388–LN1390) sit in a top-level file like LN1387; moving them with LN0004, LN1086 and LN1387 into a `files_source_missing/` module is optional.
- Upstream-signature recipes not yet re-imported with their `pre-build` hooks: repeated-signature, explicit-armor-header, doubly-armored-signature, control-file-upstream-signature-missing, and a dfsg variant (the siblings seam re-imports what it needs).

## Decisions already made

- Branch: lint work integrates onto `milo/debmagic-lint-subcommand`.
- Pointer form `[path]` is the oracle truth for `source-is-missing`, `source-contains-prebuilt-javascript-object` and `source-contains-prebuilt-python-object`; the harness normalises the bare form. The same normalisation extends to the two Python Tags (LN1571, LN1572).
- iso-codes: vendored from an explicit pinned version (4.20.1) with a regeneration script.
- `fancy-regex`, `roxmltree` and `yaml-rust2` are accepted dependencies.
- Overrides stay parsed, not applied, for now.
