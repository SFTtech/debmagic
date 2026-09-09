# Import a lintian Rule

This is the working procedure for turning a lintian Tag into an LN Rule. Native Rule tests (`SourceTreeTester` + insta) come first. Lintian’s own recipes become the Parity oracle later — they cannot be dropped into `SourceTreeTester` as a source tree.

Related: [Parity is Tag-set](../adr/0007-lintian-parity-is-tag-set.md), [oracle is the test suite](../adr/0012-parity-oracle-is-lintian-test-suite.md), [Codes when catalogued](../adr/0015-ln-codes-assigned-when-tag-is-catalogued.md), [per-Subject Rule traits](../adr/0025-per-subject-rule-traits.md), [source Rule is tree and package](../adr/0040-source-rule-is-tree-and-package.md), [Rules may read Dsc](../adr/0044-rules-may-read-dsc.md), [Rule file names](../adr/0027-rule-files-are-code-and-snake-tag.md), [Source-package Parity assembles filled trees](../adr/0039-source-package-parity-assembles-filled-trees.md), [Binary-package Parity builds filled trees](../adr/0043-binary-package-parity-builds-filled-trees.md), [recipe layout](../research/lintian-test-recipes.md).

Pin lintian **2.139.0** (`575a2bf1`). Installed tags and checks are under `/usr/share/lintian/`; recipes are **not** in that package — clone git or `apt source lintian` at that version.

---

## 1. Identify the Tag

Read `/usr/share/lintian/tags/…/<tag>.tag`:

- `Tag:` — this is our Tag string, verbatim.
- `Severity:` / experimental — default Severity and whether `default_selected` is true (error and warning, non-experimental).
- `Check:` — path of the Perl check (`fields/required` → `Lintian::Check::Fields::Required`).
- `Renamed-From:` — old names; we still key on the current Tag.

Classification tags are not Rules.

## 2. Catalogue the Rule

Add a unit struct named from the Tag (`RequiredField`) under `src/lint/rules/lintian/lnNNNN_<snake_tag>.rs` (or `src/lint/rules/native/dmNNNN_<snake_tag>.rs` for a DM Rule), `declare_rule!` with a new `LNxxxx` / `DMxxxx` Code, `impl` the subject trait for each kind the Rule inspects now (`SourceRule` for Source tree and Source package, `BinaryPackageRule` for Binary package), and register it in `registry.rs` (the metadata catalog and each kind list whose trait it implements). The filename is `{lowercase_code}_{snake_tag}.rs`; the module is that stem; `declare_rule!` is canonical. If lintian renames the Tag, `git mv` the file to the new snake Tag and leave the Code prefix. Lintian files may later split into further subfolders under `lintian/`.

Until we generate Codes from the full Tag list, assign the next free `LN` Code when the Tag is catalogued and do not reuse it.

## 3. See what the check inspects

Read the Check module. Map I/O to the Subject’s context (Debian control, changelog, file index, …). Implement only the subject traits for kinds you inspect; a kind without a trait impl is not Applicability. `SourceRule` covers both Source tree and Source package.

## 4. Native tests first

`SourceTreeTester` takes pass/fail Source trees as path → content maps and snapshots fail Diagnostics. That is **not** Parity. Use it to lock the Rule’s behaviour on a few trees (including udeb / missing-file edges the recipes never isolate). `SourcePackageTester` injects Orig files and Patched files the same way (orig defaults to empty) and an optional Dsc paragraph. `BinaryPackageTester` exists as a name; it is not implemented yet.

Keep expected extra/context in the message in lintian’s order (for `required-field`: `(in section for …)` then the field name). Pointers like `[debian/control:1]` belong on `Location` when we have them; they are stripped from `eval/hints` extras and are not part of Parity.

## 5. Find upstream recipes

Recipes live at:

```text
t/recipes/checks/<check>/
```

for example `t/recipes/checks/fields/required/`. Each recipe has `build-spec/` (how to build) and `eval/hints` (expected universal output).

`eval/desc` lists a **Check**, not a Tag (`Test-For` is gone). Selecting `tag:required-field` in lintian’s runner therefore means “every recipe for `fields/required`”. Grep `eval/hints` for the Tag as the **third field** (a substring hits `doc-base-file-lacks-required-field`).

Classify each recipe by skeleton (`build-spec/fill-values` → `Skeleton:`):

| Skeleton | Artifact | Debmagic Subject today |
| --- | --- | --- |
| `upload-native` / `upload-non-native` | `.changes` via `dpkg-buildpackage` | Source tree *before* the build; Source package / Binary package after ([ADR 0043](../adr/0043-binary-package-parity-builds-filled-trees.md)) |
| `source-native` / `source-non-native` | `.dsc` | Source package; Source tree after fill |
| `deb` | hand-built `.deb` | Binary package |
| `changes` | `.changes` | none yet |

A single `hints` file often mixes `(source)` and `(binary)` lines. Import only the lines that match the Subject you are implementing. Source-tree Parity drops `(source)` extras that name a `.dsc`. Source-package Parity includes those extras when a catalogued Tag inspects Dsc. Source-package Parity lists a filled Source tree when a catalogued `(source)` Tag has Source-package Applicability (`SourceRule`).

## 6. Why recipes are not a copy-paste into `SourceTreeTester`

A recipe is a **spec**: skeleton templates (`t/templates/…`), `[% $source %]` fills, optional `pre-build`, then usually `dpkg-buildpackage`. `build-spec/debian/` is an overlay, not a complete Source tree (there is no `debian/debian/` in 2.139.0).

The oracle is `eval/hints` in **universal** format:

```text
package (source|binary|changes|udeb): tag-name extra… [optional-pointer]
```

not EWI (`E: pkg: tag`) and not our Diagnostic stdout.

Do not submodule lintian or read `/usr/share/lintian` at test time. Native `SourceTreeTester` cases stay hand-written. Parity recipes are **filled sources** plus transcribed `eval/hints` (GPL-2+), with an `ORIGIN` pointer to `lintian 2.139.0 t/recipes/…` — not the raw `build-spec`, and not a built `.deb` / `.dsc`. They live under `packages/debmagic/tests/lintian-parity/`. Source-tree Parity lints that tree as-is. Binary-package Parity lists recipes whose `eval/hints` mention a catalogued Tag with Binary-package Applicability: `deb`-skeleton recipes assemble lintian’s Build-Product (`make` / `ar` / `tar`); filled Source trees are built with `dpkg-buildpackage -b` and each produced archive is linted ([ADR 0043](../adr/0043-binary-package-parity-builds-filled-trees.md)). Source-package Parity copies the filled tree without `ORIGIN`/`eval/`, runs `dpkg-source -b`, and on `no upstream tarball found` packs a main orig from non-`debian/` paths ([ADR 0039](../adr/0039-source-package-parity-assembles-filled-trees.md)).

Import or refresh the full Parity suite from a lintian version tag (every importable recipe: source-tree, deb, and changes):

```shell
python3 scripts/import_lintian_parity.py 2.139.0
```

Reuse a local clone:

```shell
python3 scripts/import_lintian_parity.py 2.139.0 --lintian-src /path/to/lintian
```

Import also rewrites `packages/debmagic/tests/lintian_parity.rs` with one test per vendored Source-tree, Binary-package, or Source-package recipe whose `eval/hints` mention a catalogued LN Tag (Source-package: `SourceRule`; Binary-package: `(binary)` / `(udeb)`). After cataloguing a Rule without re-filling recipes:

```shell
python3 scripts/import_lintian_parity.py --write-tests
```

## 7. Worked sketch: `required-field`

Check: `lib/Lintian/Check/Fields/Required.pm`. Tag file: `tags/r/required-field.tag`. Three recipes:

- **`generic-empty`** — `upload-native`; `control.in` omits `Standards-Version` and `Description`; `pre-build` deletes `debian/compat` and `debian/copyright`. Source extras: `(in section for source) Standards-Version` and `(in section for generic-empty) Description`. Also `.dsc` and `.deb` lines — `.deb` is Binary-package (`fields-general-missing`); `.dsc` is Source-package Dsc (`generic-empty_1.0.dsc Standards-Version`).
- **`fields-general-missing`** — `deb`; Binary package Parity (assemble the Build-Product, lint the `.deb`).
- **`changes-missing-fields`** — `changes`; no Subject yet.

`LN0001` Source-tree Parity is `generic-empty` under `tests/lintian-parity/` (two `debian/control` hint lines). Binary-package Parity is `fields-general-missing` and the built `generic-empty` `.deb`. Source-package Parity of `generic-empty` includes the Dsc extra. Source-package Parity’s first File-tree case remains `cruft-general-upstream` / LN0004.
