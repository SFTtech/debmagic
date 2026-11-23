# Import a lintian Rule

This is the working procedure for turning a lintian Tag into an LN Rule. Native Rule tests (`SourceTreeTester` + insta) come first. Lintian’s own recipes become the Parity oracle later — they cannot be dropped into `SourceTreeTester` as a source tree.

Related: [Parity is Tag-set](../adr/0007-lintian-parity-is-tag-set.md), [oracle is the test suite](../adr/0012-parity-oracle-is-lintian-test-suite.md), [Codes when catalogued](../adr/0015-ln-codes-assigned-when-tag-is-catalogued.md), [per-Subject Rule traits](../adr/0025-per-subject-rule-traits.md), [source Rule is tree and package](../adr/0040-source-rule-is-tree-and-package.md), [Rules may read Dsc](../adr/0044-rules-may-read-dsc.md), [Rule file names](../adr/0027-rule-files-are-code-and-snake-tag.md), [Source-package Parity assembles filled trees](../adr/0039-source-package-parity-assembles-filled-trees.md), [Binary-package Parity builds filled trees](../adr/0043-binary-package-parity-builds-filled-trees.md), [recipe layout](../research/lintian-test-recipes.md).

Pin lintian **2.144.0** (`3e81fac9`). Installed tags and checks are under `/usr/share/lintian/`; recipes are **not** in that package — clone git or `apt source lintian` at that version.

---

## 1. Identify the Tag

Read `/usr/share/lintian/tags/…/<tag>.tag`:

- `Tag:` — this is our Tag string, verbatim.
- `Severity:` / experimental — default Severity and whether `default_selected` is true (error and warning, non-experimental).
- `Check:` — path of the Perl check (`fields/required` → `Lintian::Check::Fields::Required`).
- `Renamed-From:` — old names; we still key on the current Tag.

Classification tags are not Rules.

## 2. Catalogue the Rule

Add a unit struct named from the Tag (`RequiredField`) under `src/lint/rules/lintian/lnNNNN_<snake_tag>.rs` (or `src/lint/rules/native/dmNNNN_<snake_tag>.rs` for a DM Rule), `declare_rule!` with a new `LNxxxx` / `DMxxxx` Code, `impl` the subject trait for each kind the Rule inspects now (`SourceRule` for Source tree and Source package, `BinaryPackageRule` for Binary package), and register it in `registry.rs` (the metadata catalog and each kind list whose trait it implements). `just new-rule <tag>` does all of this mechanically (file, module, registry), with kinds taken from the vendored recipes; `just lintian-tag <tag>` prints everything this guide has you look up by hand (Check module, API mapping, recipes, commands). The filename is `{lowercase_code}_{snake_tag}.rs`; the module is that stem; `declare_rule!` is canonical. If lintian renames the Tag, `git mv` the file to the new snake Tag and leave the Code prefix. Lintian files may later split into further subfolders under `lintian/`.

Until we generate Codes from the full Tag list, assign the next free `LN` Code when the Tag is catalogued and do not reuse it.

## 3. See what the check inspects

Read the Check module. Map I/O to the Subject’s context (Debian control, changelog, file index, …). Implement only the subject traits for kinds you inspect; a kind without a trait impl is not Applicability. `SourceRule` covers both Source tree and Source package. `(changes)` and `(buildinfo)` hints are `ChangesRule` and `BuildinfoRule`; a Check that reads `$self->group` is a `GroupRule` (`just new-rule <tag> --kind group`: the recipe hints of a group Tag are `(source)`, so it cannot be guessed). The Rule kinds a Tag has are the processable types of its hints; a Check's `always` hook is one `impl` per kind.

## 4. Native tests first

`SourceTreeTester` takes pass/fail Source trees as path → content maps and snapshots fail Diagnostics. That is **not** Parity. Use it to lock the Rule’s behaviour on a few trees (including udeb / missing-file edges the recipes never isolate). `SourcePackageTester` injects Orig files and Patched files the same way (orig defaults to empty) and an optional Dsc paragraph. `BinaryPackageTester` injects Data files, Installation control fields (`.field`), and Control files (`.control_file`, empty by default) as fixtures, and can mark the case a udeb.

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
| `upload-native` / `upload-non-native` | `.changes` via `dpkg-buildpackage` | Source tree *before* the build; Source package / Binary package after ([ADR 0043](../adr/0043-binary-package-parity-builds-filled-trees.md)); Group (the `.changes`, `.dsc`, `.deb`s and `.buildinfo`) for `(changes)`, `(buildinfo)` and group-Check hints ([ADR 0055](../adr/0055-group-rule-and-changes-buildinfo-subjects.md)) |
| `source-native` / `source-non-native` | `.dsc` | Source package; Source tree after fill |
| `deb` | hand-built `.deb` | Binary package |
| `changes` | `.changes` | Group (a `.changes` and the files next to it) |

A single `hints` file often mixes `(source)` and `(binary)` lines. Import only the lines that match the Subject you are implementing. Source-tree Parity drops `(source)` extras that name a `.dsc`. Source-package Parity includes those extras when a catalogued Tag inspects Dsc. Source-package Parity lists a filled Source tree when a catalogued `(source)` Tag has Source-package Applicability (`SourceRule`).

## 6. Why recipes are not a copy-paste into `SourceTreeTester`

A recipe is a **spec**: skeleton templates (`t/templates/…`), `[% $source %]` fills, optional `pre-build`, then usually `dpkg-buildpackage`. `build-spec/debian/` is an overlay, not a complete Source tree (there is no `debian/debian/` in 2.144.0).

The oracle is `eval/hints` in **universal** format:

```text
package (source|binary|changes|udeb): tag-name extra… [optional-pointer]
```

not EWI (`E: pkg: tag`) and not our Diagnostic stdout.

Do not submodule lintian or read `/usr/share/lintian` at test time. Native `SourceTreeTester` cases stay hand-written. Parity recipes are **filled sources** plus transcribed `eval/hints` (GPL-2+), with an `ORIGIN` pointer to `lintian 2.144.0 t/recipes/…` — not the raw `build-spec`, and not a built `.deb` / `.dsc`. They live under `packages/debmagic/tests/lintian-parity/`. Source-tree Parity lints a copy of that tree without `ORIGIN` and `eval/`. Binary-package Parity lists recipes whose `eval/hints` mention a catalogued Tag with Binary-package Applicability: `deb`-skeleton recipes assemble lintian’s Build-Product (`make` / `ar` / `tar`); filled Source trees are built with `dpkg-buildpackage -b` and each produced archive is linted ([ADR 0043](../adr/0043-binary-package-parity-builds-filled-trees.md)). Source-package Parity copies the filled tree without `ORIGIN`/`eval/`, runs `dpkg-source -b`, and on `no upstream tarball found` packs a main orig from non-`debian/` paths ([ADR 0039](../adr/0039-source-package-parity-assembles-filled-trees.md)).

Three `build-spec` pieces a filled tree cannot show are kept under `<recipe>/build-spec/` and read by the harness (a copy of the recipe leaves the directory out): `dpkg-overrides` (the `-DField=value` words lintian passes to `dpkg-source`; Source-package Parity passes them on, Source-tree Parity sets the field in the Source paragraph of `debian/control`) and `pre-upstream.in`, only when the hook creates `debian/` in the orig (the tree's `debian/` is the packaged one). With that hook the harness packs the orig as lintian's upload builder does: the tree's upstream paths plus the hook output under `<source>-<upstream>/`, so a 1.0 package gets a real `.diff.gz`. `pre-build` is kept (the filled hook) only when it makes orig tarballs next to the source directory (the `signed`, `noext` and `component` origs and their `.asc` files of the `dep5/components` recipes): the harness runs it on the packed source directory before `dpkg-source -b`, so the `.dsc` lists the components. Source-package Parity packs an orig for every recipe of a non-native skeleton (`upload-non-native`, `upload-builder-only`, `source-non-native`), as lintian's builder does, even when the recipe has no upstream files: without it `dpkg-source` builds a 1.0 package as a native one (no `.diff.gz`) and the non-native Tags do not fire. A hint that compares Patched files with Orig files (`no-debian-changes`) is expected only from the Source package: a Source tree has no orig. A `pre-build` that re-signs or re-armors the orig also needs `build-spec/upstream/private-key.asc`, which the harness imports into a throwaway GnuPG home to sign the packed orig as lintian's upload builder does. These keys are lintian's own public test keys (test material, not a secret), vendored under `build-spec/upstream/` for `upstream-signature/repeated-signature`, `doubly-armored-signature` and `explicit-armor-header`. To refresh just these pieces, `python3 scripts/import_lintian_parity.py 2.144.0 --lintian-src <clone> --recipe <path under t/recipes>` (repeatable; leaves `PIN` alone).

lintian 2.144.0's `files/source-missing` Check reports `source-is-missing`, `source-contains-prebuilt-javascript-object` and `source-contains-prebuilt-python-object` only as `pointed_hint(tag, $item->pointer)`: an empty extra and the path as the pointer (`tag [path]`). The recipes `checks/cruft/cruft-general-upstream` and `checks/cruft/cruft-python` still carry the bare `tag path` from an older lintian, which the pinned code cannot produce. The pointer form is the truth: the harness (`point_bare_source_missing_hints` in `tests/common/mod.rs`) reads a bare path extra of exactly these three Tags as an empty extra with that path as Location, and leaves every other Tag and the vendored hint files alone. Those two recipes also hold byte-compiled files whose names depend on the Python that ran the recipe's `pre-build` (`__pycache__/fake.cpython-314.pyc` on a current host, `fake.pyc` and `fake2.pyo` for the Python 2 of the hints), so their vendored `.pyc`/`.pyo` files are hand-set to the names the hints expect (valid Python byte-code headers); re-importing them regenerates host-dependent names and breaks Parity.

lintian 2.144.0's `languages/python/obsolete` Check reports `debian-pycompat-is-obsolete` and `debian-pyversions-is-obsolete` as `pointed_hint(tag, $file->pointer)` on `debian/pycompat` and `debian/pyversions`. The `cruft-python` recipe still carries them with an empty extra and no path, and the bare `tag path` form elsewhere, from an older lintian. The pointer form is the truth: the harness (`point_python_obsolete_hints` in `tests/common/mod.rs`) reads an empty or bare-path extra of exactly these two Tags, and only these two paths, as `[path]` (the Location with no extra), the form `legacy-debconf` already prints. The Rules report each file with that Location and no extra.

Recipes ship cruft on purpose (editor backups, bytecode, VCS directories), so `.gitignore` does not ignore anything under `tests/lintian-parity/`. Git cannot track a directory named `.git`, so the importer vendors those as `.git.vendored` and the Parity harness restores `.git` when it copies a recipe.

Import or refresh the full Parity suite from a lintian version tag (every importable recipe: source-tree, deb, and changes):

```shell
python3 scripts/import_lintian_parity.py 2.144.0
```

Reuse a local clone:

```shell
python3 scripts/import_lintian_parity.py 2.144.0 --lintian-src /path/to/lintian
```

A refresh rewrites recipes, so wipe the suite first (`git rm -r packages/debmagic/tests/lintian-parity`) to drop recipes lintian removed, then re-add what the importer cannot reproduce:

- Hooks that need tools missing here (`pre-build` calling `terser` in the `cruft-minified-js` and `min-js-with-sources` recipes) leave their output out. Restore those files from the previous import.
- Hand-maintained recipes, noted in their `ORIGIN`: `checks/debian/patches/quilt/patch-systems-quilt-general` (the `debian-changes-1.0-1` patch dpkg-source auto-commits, plus its `series` line) and `odd-inputs/source/debian-symlink` (files at the top level with `debian` a symlink to `.`).

Import also rewrites `packages/debmagic/tests/lintian_parity.rs` with one test per vendored Source-tree, Binary-package, Source-package, or Group recipe whose `eval/hints` mention a catalogued LN Tag (Source-tree: Tags of Source and Binary-package Rules; Source-package: `SourceRule`; Binary-package: `(binary)` / `(udeb)`; Group: a recipe whose product is a `.changes` (`upload-*`, `changes`) with `(changes)` hints of `ChangesRule` Tags, `(buildinfo)` hints of `BuildinfoRule` Tags, and `(source)` hints of `GroupRule` Tags). Group Parity builds the recipe, lints its `.changes` as lintian's runner does, and compares the hints per processable type. After cataloguing a Rule without re-filling recipes:

```shell
python3 scripts/import_lintian_parity.py --write-tests
```

## Where are we? `just lint-parity`

```shell
just lint-parity                      # Rule, Check, and oracle-hint coverage; fidelity flags
just lint-parity --run                # also run the generated Parity tests and show pass/FAIL per Rule
just lint-parity --candidates 30      # rank uncatalogued default-selected Tags by oracle weight
just lint-parity --json --strict      # machine-readable; exit 1 on any listed issue
```

It reads the lintian tag catalogue (`--lintian-data`, default `/usr/share/lintian`), `declare_rule!` / `impl SourceRule` / `impl BinaryPackageRule` in `src/lint/rules/`, the vendored `eval/hints`, and the generated `tests/lintian_parity.rs`. Flags worth acting on: `applicability-gap` (the oracle expects the Tag from a processable type the Rule does not implement), `severity` / `default-selected` mismatches against lintian, `no Parity case` (a Rule whose oracle never exercises the Subjects it implements), and a stale generated test file. The tag catalogue is whatever lintian is installed unless pointed elsewhere; it can drift from the pinned oracle.

### Real-world check: the ecosystem run

Parity is the vendored oracle. To see the Rule on real Debian archive packages, and to get a
self-contained fix packet per disagreeing Rule, run the ecosystem suite — see
[Ecosystem run](lint-ecosystem.md). It writes `target/lint-ecosystem/rules/<CODE>_<tag>.md`.

### The Parity build cache

Most of the time in `cargo test -p debmagic --test lintian_parity` is spent building the recipes (`dpkg-buildpackage` for Binary packages, `dpkg-source` for Source packages), not linting them. The harness (`tests/common/mod.rs`) therefore builds each distinct recipe tree once and keeps the built products, never lint results, under `target/lint-parity-cache/<key>/` (a `cargo clean` clears it, and two checkouts cannot share or poison each other). Linting always runs, so changing a Rule is seen by the next run: the cache holds what was built, not what the Rules said about it.

- What is cached: the Binary-package builds (`.deb`s of `dpkg-buildpackage` recipes and of `deb` skeletons) and the Source-package builds (`.dsc`, `.orig`, and `.debian` tarballs). Group builds (`upload-*`, `changes` skeletons) are not cached: their `.buildinfo` records the build path, date and the host's `Build-Tainted-By`, so two builds are not interchangeable.
- The key is a SHA-256 over the copied tree (what the tests build: no `ORIGIN`, `eval/`, `build-spec/`, with `.git.vendored` restored to `.git`; names, modes, contents, link targets) and everything else that changes the product: the build kind, `DEB_BUILD_OPTIONS` and `dpkg-buildpackage` arguments, the contents of the `dpkg-deb --nocheck` wrapper, whether the `dh --with nodejs` stub is used and its contents, the `dpkg-source` flags, overrides, `debian-adds-` exclusions and `pre-upstream` hook of a Source package, the host architecture, the uid and gid, the `DEB_*` / `DH_*` / `DPKG_*` / `LC_*` / `LANG` / `TZ` / compiler-flag / `SOURCE_DATE_EPOCH` environment variables, the installed packages (a hash of `/var/lib/dpkg/status`, standing for the toolchain), and a format version. Two recipes that differ only in `eval/hints` or `eval/desc` share one build; touching a recipe's files rebuilds only that tree. Anything that upgrades a package invalidates every entry.
- A recipe that fails to build fails only its own test: a failure is neither cached in memory nor on disk, and the next recipe with the same tree builds it itself.
- Entries are written under a `.tmp-` name and renamed into place, so concurrent test processes and killed runs leave no half-written entry; a missing, truncated or altered entry (every file is checked against a SHA-256 manifest) is removed and rebuilt. Leftover `.tmp-` directories older than an hour are deleted.
- `DEBMAGIC_PARITY_NO_CACHE=1 cargo test -p debmagic --test lintian_parity` bypasses the cache completely (nothing read, nothing written, no sharing between recipes): every recipe builds its own tree. Use it to rule the cache out when a result looks wrong, or after changing a tool the key does not see (`PATH`, a tool in `/usr/local`).
- To drop the cache by hand: `rm -r target/lint-parity-cache`.

### Oracle conflicts: expected-side drops

Parity compares a Rule with lintian's `eval/hints`, and sometimes the oracle expects a hint lintian 2.144.0 cannot emit for that recipe. Such a conflict is recorded as an explicit, commented drop on the **expected side only**, scoped to one recipe, never a skip. The thirteen `source-contains-*` Tags of `files/artifact` are the first: `checks/cruft/cruft-general-upstream` lists them, but its vendored build is non-native (`debian/source/format` is `3.0 (quilt)`), so lintian emits `debian-adds-*` there. `drop_expected_hints_for_non_native_upstream` in `packages/debmagic/tests/common/mod.rs` drops exactly those Tags from that recipe, and the native oracle `checks/files/artifact/cruft-general-native` still compares them. The importer (`scripts/import_lintian_parity.py`) transcribes `eval/hints` verbatim and does not apply drops, so a new drop goes into the harness only. Add a drop only when lintian 2.144.0's own Check cannot produce the hint, and say why in the comment.

## 7. Worked sketch: `required-field`

Check: `lib/Lintian/Check/Fields/Required.pm`. Tag file: `tags/r/required-field.tag`. Three recipes:

- **`generic-empty`** — `upload-native`; `control.in` omits `Standards-Version` and `Description`; `pre-build` deletes `debian/compat` and `debian/copyright`. Source extras: `(in section for source) Standards-Version` and `(in section for generic-empty) Description`. Also `.dsc` and `.deb` lines — `.deb` is Binary-package (`fields-general-missing`); `.dsc` is Source-package Dsc (`generic-empty_1.0.dsc Standards-Version`).
- **`fields-general-missing`** — `deb`; Binary package Parity (assemble the Build-Product, lint the `.deb`).
- **`changes-missing-fields`** — `changes`; a Changes file (`ChangesRule`).

`LN0001` Source-tree Parity is `generic-empty` under `tests/lintian-parity/` (two `debian/control` hint lines). Binary-package Parity is `fields-general-missing` and the built `generic-empty` `.deb`. Source-package Parity of `generic-empty` includes the Dsc extra. Source-package Parity’s first File-tree case remains `cruft-general-upstream` / LN0004.
