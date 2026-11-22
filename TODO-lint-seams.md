# Open lint seams

A *seam* is a Rule-facing inspectable, parser, or harness capability that a Tag needs and we do not have yet. This file is the ledger: every open seam with the Tags it blocks, plus a short record of what landed. It is updated whenever an agent reports a new seam, a seam changes status, or a seam lands. The per-inspectable narrative stays in [TODO-lint.md](TODO-lint.md).

**Last updated**: 2026-11-22 (evening), milo tip `82cd78e3`, 1216 LN Rules.
**Measure**: `just lint-parity` (`--candidates N` for ranked next Rules).

**Status words**: *In progress* (an agent is on it), *Approved* (the user agreed to the design, not started), *Proposed* (written up, waiting for a go), *Needs decision* (a design or policy question for the user), *Deferred* (decided not to do now), *Landed* (in milo).

## Open seams

| Seam | Status | Blocked Tags | Notes |
| --- | --- | --- | --- |
| Parity harness: pack non-native source packages like lintian's upload builder (orig and component tarballs, `.diff.gz`) | In progress (`lint-harness`) | `upstream-metadata-file-is-missing` (LN1338); 8 source-package recipes of the YAML seam (`basic-non-native-1.0`, `generic-dh-make-2005/2008`, four `watch-file-missing-with-*`); dep5 `components/control-file-upstream-signature-missing` (host-skipped today) | The YAML branch merges after this lands. |
| Importer: `read_deb822` flattens multi-line fields | In progress (`lint-harness`) | Possibly wrong content in other imported recipes (batch 8 repaired `rust/template-description` by hand) | Re-import only the affected recipes. |
| Oracle form: `[path]` pointer versus bare path extra | In progress (`lint-harness`) | `source-is-missing` (LN1390), `source-contains-prebuilt-javascript-object` (LN1388), `source-contains-prebuilt-python-object` (LN1389) | Decided: pointer form is the truth, normalise in the harness for these three Tags. |
| Group Rules may report on an installable (hints attach to the binary package, not the source) | In progress (`lint-n100-group`) | `usr-share-doc-symlink-to-foreign-package`, `missing-notice-file-for-apache-license`, `gir-missing-typelib-dependency`, `doc-base-file-references-missing-file`, `package-contains-broken-symlink-wildcard`, `lacks-unversioned-link-to-shared-library` | Design question inside the group batch; the agent reports the extension. |
| Group harness: `Upload-Type: source` recipes need `dpkg-buildpackage -S` | Proposed | one Tag skipped by batch `misc-0` (see its report); `source-*` skeletons whose product is a `.changes` | Harness agent has it as a stretch item. |
| Group Rules reporting with processable type `changes` (or `ChangesContext` seeing the source's `.dsc` fields); the Parity generator and `applies_to_group` attribute `changes` hints only to `ChangesRule`/`BuildinfoRule` | Proposed | `inconsistent-maintainer` (LN1491, needs the `.dsc` Maintainer but reports a `changes` hint); possibly other Maintainer checks on the `.changes` side | Small, harness plus context. Found by batch `changes-b`. |
| Group harness: `ARCHITECTURE` placeholder rewrite for buildinfo recipes | Proposed | `package-has-long-file-name` (buildinfo hint) | Small, harness only. |
| MD5 digest (`Entry::md5sum()`, an md5 crate) | Approved | `license-problem-md5sum-non-distributable-file`, `license-problem-non-free-img-lenna` (also needs content sniffing), the checksum Tags of `.changes` files (`checksum-mismatch-in-changes-file`, `checksum-count-mismatch-in-changes-file`) | sha1 and sha2 already exist. |
| Content-type sniffing (images, PDF, PostScript; libmagic-class, no `file(1)`) | Approved | `license-problem-non-free-img-lenna` | Several Rules sniff privately today (image names, ar, RTF); could be shared. |
| Compressed-stream integrity check (bzip2, lzip, lzop) | Approved | `broken-bz2`, `broken-lz`, `broken-lzo` | Pure-Rust decoder or the external tools. |
| Type 1 font parser (PFA/PFB, eexec, `t1disasm` text) | Approved | `license-problem-font-adobe-copyrighted-fragment`, `license-problem-font-adobe-copyrighted-fragment-no-credit` | |
| External tool runner (materialise a File-tree subset in a temp dir, run a host tool, capture output) | Approved | `shell-script-fails-syntax-check`, `maintainer-shell-script-fails-syntax-check` (`dash -n` / `bash -n`), `invalid-potfiles-in`, `newer-debconf-templates` (`intltool-update`, `msgcmp`), `appstream-metadata-validation-failed` (`appstreamcli validate-tree`; its recipe fixture reports "Passed" on appstreamcli 1.2.3, so a host skip is likely) | The host may lack gettext and intltool; host skips follow `tests/common/mod.rs`. |
| Sibling files of a `.dsc` (names plus lazy bytes of the `.dsc`'s directory, lintian's `processable->files`) | Proposed | `concatenated-upstream-signatures`, `doubly-armored-upstream-signature`, `explicitly-armored-upstream-signature`, `orig-tarball-missing-upstream-signature`, `spurious-fields-in-upstream-signature` | Distinct from the Group seam's sibling `.deb` lookup. |
| Tar unpack messages (warnings the tar reader emits per Data or Control tarball) | Proposed | `unpack-message-for-deb-data`, `unpack-message-for-deb-control` | Also the source and orig unpack Tags later. |
| Orig-file mtimes (host trees from filesystem metadata, or read orig tarball headers) | Proposed | `orig-tarball-contains-ancient-file` (LN1461) | `Entry::mtime()` exists for Binary packages. |
| ELF symbol-table fact (`ElfFacts::has_symbol_table()`) | Proposed | `ocaml-custom-executable` (LN1401) | The strings helper it also needs has landed. |
| iso-codes data: pinned version plus a regeneration script | Decided (recommended option), not started | `incorrect-locale-code` (LN1385), `locale-uses-language-group-code` | `unknown-locale-code` (LN1386) already vendors the host's iso-codes 4.20.1; the script pins an explicit version and regenerates both tables. |
| Source's control visible to Binary-package Rules (lintian's group access from binary Checks) | Proposed | exact behaviour of `program-not-linked-against-libc` (octave skip), Go skips in hardening Rules | Today a Binary package Subject has no source package; a Group run could supply it. |
| Group behaviours: a bad argument skips and continues (lintian) instead of aborting the run | Proposed (my recommendation: follow lintian) | none (behaviour only) | Reported by the group seam, ADR 0054. |
| Group Parity for `source-*` recipes (product is a `.dsc` only) | Proposed | group hints of those recipes are not covered | From the group seam's open questions. |
| Policy-release table refresh to the 2.144.0 pin | Proposed | `out-of-date-standards-version` accuracy | The shared table is from lintian 2.139.0; stale comment in `lint/data/mod.rs` too. |
| Override application pass (suppress Diagnostics with parsed overrides) | Deferred | `mismatched-override`, `unused-override` | A design change, ADR 0051 says parsed, not applied. Revisit when wanted. |

## Not seams: Tags with no usable oracle

- `debhelper-compat-not-a-number` (LN1305): lintian 2.144.0 never emits it; the recipe cannot be matched natively. Drop from the plan.
- `elf-warning` (LN1460): no vendored recipe, so no Parity; would also need readelf WARNINGS modelled.

## Landed in milo

| Seam | Landed with |
| --- | --- |
| Multi-artifact runs: Group, `.changes` and `.buildinfo` Subjects, `ChangesRule` / `BuildinfoRule` / `GroupRule`, testers, `new-rule --kind`, Parity harness builds Groups (ADR 0054, 0055) | LN1440, LN1441; LN0374 for Changes and Buildinfo |
| `debian/control` field positions (`ControlParagraph::position`), raw `resembles` tables | LN1312–LN1315 |
| XML documents (`roxmltree`) and AppStream metadata | LN1320, LN1462–LN1466 |
| Spelling module and printable-strings view (`fancy-regex` for lookahead patterns) | LN1301, LN1310, LN1311, LN1423, LN1468, LN1408 |
| `debian-binary` bytes and any-ar-archive reader, `Entry::mtime()`, `BinaryPackageContext::installed_changelog()` | LN1304, LN1382, LN1418 |
| ELF facts (section names, machine, class, endianness, readelf-style errors) and shared `ar` parser | LN1290–LN1293, LN1416 |
| Parity build cache (catalogue scan caching, in-process tree dedupe, persistent cache under `target/lint-parity-cache`; cold about 70 s, warm about 10 s) | n/a (test infrastructure) |

## Landed on a branch, waiting for its merge

| Seam | Branch | Waiting for |
| --- | --- | --- |
| Upstream metadata YAML (`ctx.upstream_metadata()`, `yaml-rust2`) | `lint-seam-yaml` | the harness fix (8 source-package recipes fail without non-native packing) |

## Decisions already made

- Branch: lint work integrates onto `milo/debmagic-lint-subcommand`.
- Pointer form `[path]` is the oracle truth for the three cruft Tags; normalise in the harness.
- iso-codes: vendored from an explicit pinned version with a regeneration script.
- `fancy-regex` and `roxmltree` and `yaml-rust2` are accepted dependencies.
- Overrides stay parsed, not applied, for now.
