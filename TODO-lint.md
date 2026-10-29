# Next: lint inspectables

**Status**: slices 1 (Dsc) and 3 (Control files) done. See [ADR 0044](docs/adr/0044-rules-may-read-dsc.md) and [ADR 0045](docs/adr/0045-binary-package-control-files-is-a-file-tree.md).
**Measure**: `just lint-parity` (`--run` for Parity results, `--candidates N` for ranked next Rules).
**Related**: [What lintian rules need to inspect](docs/research/lintian-rule-information-access.md), [import a lintian Rule](docs/develop/import-lintian-rule.md), leftover in [TODO-source-package-parity.md](TODO-source-package-parity.md).

## Goal

Grow the Rule-facing inspectables — one seam per slice, one forcing Tag, native tests plus Parity for that Tag. Rules that already fit Debian control, today's File tree, or File type stay out.

## Work

1. [x] **Dsc** — Expose the parsed `.dsc` paragraph on SourceContext (absent on a Source tree). Forcing Tag: `required-field` (LN0001 `.dsc` extras; Source-package Parity for `generic-empty`). Do not invent Dsc from patched Debian control. ADR 0044: Rules may read Dsc; extract identity stays private.
2. [~] **File-tree metadata** — Entry grows mode, owner, size, and symlink target. Host trees and Data files both emit directory and symlink kinds (Data files are regular files only today). **Done so far**: symlink target on host trees and Data files; Data files emit directories and symlinks; `package-contains-unsafe-symlink` is LN0006 with both Binary Parity recipes. **Also done**: Binary-package Data files emit tar hardlinks (content of the link target) and keep backslashes in member names. **Left**: mode, owner, size, each when a forcing Tag needs it. The executable bit is mode, not File type. Forcing Tag: `package-contains-unsafe-symlink` (Binary; 2 recipes). Same seam, Source-side oracle: `wayward-symbolic-link-target-in-source` / `absolute-symbolic-link-target-in-source` (`cruft-unsafe-symlinks`).
3. [x] **Control files File tree** — Expose Control files on Binary package the same way as Data files. Installation control stays the parsed `control` file among those Entries. Forcing Tag: `maintainer-script-empty` is LN0014 (3 Binary recipes). See [ADR 0045](docs/adr/0045-binary-package-control-files-is-a-file-tree.md). `unknown-control-file` has no recipe in the vendored suite, so it cannot have Parity. Follow-on once mode lands: `control-file-has-bad-permissions`.
4. [~] **Debian changelog entries** — Same object as LN0002; keep syntax errors; store ordered entries (source, version, distributions, date, maintainer, change text). **Done**: `DebianChangelog::entries()` (heading line, source, version, distribution, maintainer, date, timestamp, change text; entries cut short by syntax errors keep what was read; a weekday that does not fit the date no longer fails the date parse, as in `Date::Parse`); forcing Tag `latest-debian-changelog-entry-without-new-date` is LN0026 (2 Source-tree and 2 Source-package recipes). **Left**: a parsed version (epoch, upstream, revision, NMU) for the follow-ons. The binary twin `latest-changelog-entry-without-new-date` needs a Binary-package changelog first. Follow-ons: `epoch-change-without-comment` (3 recipes), the `nmu` pair (5 recipes; also reads Debian control).
5. [~] **Debian copyright** — Parsed `debian/copyright` (plain or copyright-format 1.0). Syntax errors stay on the object. It is not Debian control. **Done**: `DebianCopyright` (format detection, Deb822 paragraphs with field line numbers, syntax errors on the value), `SourceContext::debian_copyrights()` and `shipped_files()` (lintian's shipped names and Orig-directory lookup); forcing Tag `superfluous-file-pattern` is LN0008 (10 Source-tree and 10 Source-package recipes); `no-dep5-copyright` is LN0009 (9 recipes, including the extra `debian/*.copyright` files). **Left**: format-URI and field checks (the object knows the format, not the URI text), PGP-armoured files, and the Files-Excluded handling. Next forcing Tag: `missing-license-paragraph-in-dep5-copyright` (Source; `source-copyright-dep5-general`). `syntax-error-in-dep5-copyright` has one hint line. Follow-ons: `unused-license-paragraph-in-dep5-copyright`, `dep5-copyright-license-name-not-unique`.
6. [~] **Package relation** — Policy 7.1 parser over relationship fields on Debian control, Installation control, and Dsc. A parse failure stays on the value; it does not make the control object unavailable. **Done**: `PackageRelation` (comma and `|` split as lintian's `_split_dep`, with package, `:arch` qualifier, version constraint, architecture restriction, build profiles, and leftover text per alternative), reachable as `ControlParagraph::relation(field)` on all three. Forcing Tag `package-relation-with-self` is LN0027 (2 Binary recipes). `bad-relation` has no recipe in the vendored suite. **Left**: a `satisfies` query (needed by `no-versioned-debhelper-prerequisite`, `debhelper-but-no-misc-depends`), version comparison, and the Debian control source-side fields.
7. **Interpreter table** — Vendored, lintian-2.139.0-pinned map from shebang interpreter to Depends package. Rules read `#!` from Entry bytes; no Script type. Forcing Tag: `missing-dep-for-interpreter` (Binary package Data files + Package relation).

## Later

Not sequenced. Still inspectables, not extra Rules on today's surface:

- `debian/source/` + patches (Format, quilt series, DEP-3)
- [~] **Debhelper facts** — Declared compat level (`debian/compat`, `Build-Depends: debhelper-compat (= N)`, `X-DH-Compat`) and whether `debian/rules` invokes dh. Forcing Tag: `package-uses-deprecated-debhelper-compat-version` is LN0019 (6 Source-tree recipes and 7 Source-package recipes; `debhelper-compat-missing` is Source-package only, because `tar-ignore` drops `debian/compat` from the packed source). **Left**: dh addons, and lifting LN0003's `debian/rules` scan into this object.
- ELF facts beyond File type (NEEDED, RPATH, SONAME, hardening)

## Out

- Walking Orig files — the File tree already exists
- `.changes` Subject — ADR 0016
- Overrides — `debian/lintian-overrides` still waits
