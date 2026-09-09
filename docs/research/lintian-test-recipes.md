# How lintian’s test recipes work (import notes)

Primary source: Debian lintian **2.139.0** git tag (`575a2bf1bc98500bbc1c78e82f44b40d195d17ea`), cloned from [salsa.debian.org/lintian/lintian](https://salsa.debian.org/lintian/lintian). Installed binary package **2.139.0** on this machine is `/usr/share/lintian/` and does **not** ship `t/` (see §7).

Salsa permalinks below use the `2.139.0` tag (same tree as the clone). Line numbers are from that tree.

This note is for importing **per-tag recipes** into debmagic as native SourceTreeTester fixtures (plain source tree + expected Tags), per [ADR 0012](../adr/0012-parity-oracle-is-lintian-test-suite.md).

---

## 1. Directory layout

### `t/recipes` vs older `t/tests`

At 2.139.0 there is **no** `t/tests/` directory. The only tag-testing suite is `t/recipes/` ([`lib/Test/Lintian/Filter.pm`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/lib/Test/Lintian/Filter.pm) lines 63–64: `@LINTIAN_SUITES = qw(recipes)`).

The old layout is still described in stale docs:

- [`doc/tutorial/Lintian/Tutorial/WritingTests.pod`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/doc/tutorial/Lintian/Tutorial/WritingTests.pod) lines 8–9, 63–64, 98–100, 149–163, 369–373: tests live in `t/tests/<test-name>/` with a root `desc`, a `tags` file, and a **double `debian/`** (`t/tests/<name>/debian/debian/rules`).
- [`private/runtests`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/private/runtests) lines 25–28 still points at `t/tests/README` (that file is gone).
- The current harness **rejects** leftover double-debian specs: [`lib/Test/Lintian/Prepare.pm`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/lib/Test/Lintian/Prepare.pm) lines 124–125 (`die` if `$specpath/debian/debian` exists).
- `debian/changelog` still talks about `t/tests/*` in old entries (e.g. around the 2.5.x era); that is history, not the 2.139.0 tree.

Current contributor docs are [`CONTRIBUTING.md`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/CONTRIBUTING.md) (lines 104–167) and a **partially stale** [`t/recipes/README`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/t/recipes/README) (still describes root `desc`/`tags`/`debian/` as if they were the recipe on disk). Trust CONTRIBUTING + the Perl runner over that README when they disagree.

### What is actually on disk (2.139.0)

```
t/
  recipes/          1458 tests, each identified by eval/desc
    checks/         mirrors lib/Lintian/Check/ path
    general-false-positives/
    lintian-features/
    odd-inputs/
    runner-features/
    tracking/
  skeletons/        layout + builder for a test working directory
  templates/        files copied/filled into that directory
  defaults/         default eval + fill-values + filename map
  whitelists/       which *.in templates may be generated
  scripts/          prove(1) unit tests (not tag recipes)
```

Recipe count: 1458 `eval/desc` files under `t/recipes/` (this clone). CONTRIBUTING says the recipe directory should mirror the check path, e.g. `t/recipes/checks/fields/required/<test-name>/` ([`CONTRIBUTING.md`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/CONTRIBUTING.md) lines 51–56, 109–157).

### One recipe

Each recipe is a directory containing **two** specs ([`CONTRIBUTING.md`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/CONTRIBUTING.md) 114–159):

| Path | Role |
| --- | --- |
| `build-spec/` | How to **build** the subject |
| `build-spec/fill-values` | Deb822: `Skeleton`, `Testname`, `Description`, optional `Version`, `Type`, extra depends |
| `build-spec/debian/` | Overlay onto the skeleton’s `debian/` (source/upload skeletons) |
| `build-spec/orig/` | Upstream tree for non-native / extra files (631 recipes have this) |
| `build-spec/DEBIAN/` | Binary control overlay (`Skeleton: deb`) |
| `build-spec/pre-build` | Executable hook; 94 recipes |
| `build-spec/pre-upstream` | Non-native tarball hook; 10 recipes |
| `eval/` | How to **run** lintian and what to expect |
| `eval/desc` | Deb822: `Testname`, `Check`, optional `Test-Against`, `Options`, `Profile`, `Match-Strategy` |
| `eval/hints` | Expected universal-format hint lines (oracle) |

There is **no** on-disk `tags` file anywhere under `t/recipes/` (count 0). Expected output is `eval/hints`.

`t/defaults/files` maps logical names to those paths ([`t/defaults/files`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/t/defaults/files)): `Test-Specification: desc`, `Fill-Values: fill-values`. Discovery: any `desc` whose parent’s parent is the recipe (`find_all_testpaths` walks `desc` then `parent->parent`, [`Filter.pm`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/lib/Test/Lintian/Filter.pm) 270–276).

Hooks still supported by the runner (README names, actual filenames after copy): `pre-build`, `pre-upstream`, `post-test` (sed; 270 recipes), `test-calibration` (2 recipes), `skip` (supported in [`Run.pm`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/lib/Test/Lintian/Run.pm) 213–218 and [`Build.pm`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/lib/Test/Lintian/Build.pm) 91–97; **zero** `skip` files in 2.139.0 recipes).

Preparation ([`Prepare.pm`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/lib/Test/Lintian/Prepare.pm) 85–183, 314–384):

1. Load `t/defaults/fill-values` then overlay `build-spec/fill-values`.
2. Load `t/skeletons/<Skeleton>` (`Template-Sets`, `Fill-Targets`, `Type`, `Version`).
3. Copy template sets from `t/templates/` into a work dir (`debian/test-out/…`).
4. Copy the whole recipe (including `build-spec/`) on top; drop templates that the recipe already provides.
5. Fill `*.in` via `Text::Template` (`[% $field %]`).
6. `filleval` does the same for `eval/` using skeleton `testing` by default.

---

## 2. How a recipe declares which tags it cares about

### Current mechanism (2.139.0): `Check:` + `eval/hints`

**`Test-For` does not appear** in any 2.139.0 `eval/desc` or `fill-values` (grep count 0). The README’s `Test-For:` field is leftover from `t/tests`.

What is used:

1. **`Check:` in `eval/desc`** — which lintian check module(s) to run. CONTRIBUTING: “Most tests only run a specific lintian 'check'.” and “Only tags from the selected 'check' should be included” in `hints` ([`CONTRIBUTING.md`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/CONTRIBUTING.md) 109–160). Default if omitted: `Check: all` from [`t/defaults/desc`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/t/defaults/desc) line 1. No recipe at 2.139.0 sets `Check: all` explicitly; 34 recipes omit `Check` (mostly `general-false-positives` / runner tests) and inherit `all`. **Zero** recipes list more than one check name.

2. **`eval/hints`** — the exact expected emissions. The runner’s `Match-Strategy: hints` (default, [`t/defaults/desc`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/t/defaults/desc) line 7) compares this file to lintian’s universal output ([`Run.pm`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/lib/Test/Lintian/Run.pm) 311–320, 376–412).

3. **`Test-Against:`** — still present on many `eval/desc` files. Used by `find_all_tags` to add tags that must **not** appear, and by `check_result` together with the check’s full tag list ([`Filter.pm`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/lib/Test/Lintian/Filter.pm) 328–365; [`Run.pm`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/lib/Test/Lintian/Run.pm) 488–547).

4. **Computed Test-For** — intersection of expected hint tag names and tags belonging to `Check:` ([`Run.pm`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/lib/Test/Lintian/Run.pm) 516–518). Missing those tags fails the test; unexpected tags from the rest of the check also fail.

### Format of expected hint lines (the old `tags` file)

Old EWI-style `tags` files looked like `I: pkg: tag-name` ([WritingTests.pod](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/doc/tutorial/Lintian/Tutorial/WritingTests.pod) 149–156). Current **universal** format is parsed by [`Test::Lintian::Output::Universal::parse_line`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/lib/Test/Lintian/Output/Universal.pm) lines 137–147:

```
^(\S+)\s+\(([^)]+)\):\s+(\S+)(?:\s+(.*))?$
```

That is, **one line per emission**:

```
<package> (<type>): <tag-name> [<extra/context>…]
```

- `package` — processable name (often `Testname`).
- `type` — `source` | `binary` | `changes` | `udeb` (lintian group types).
- `tag-name` — e.g. `required-field`.
- remainder — extra/context: field name, `(in section for …)`, `[debian/control:N]`, `.dsc`/`.deb` basename, etc.

Default lintian CLI includes `--exp-output format=universal` ([`t/templates/lintian-invocation/fill-values.d/lintian-invocation.values`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/t/templates/lintian-invocation/fill-values.d/lintian-invocation.values); [`t/defaults/desc`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/t/defaults/desc) lines 5–8). A few recipes set `Output-Format: EWI` and `Match-Strategy: literal` (e.g. traversal tests).

Lines are compared after a reverse sort keyed by package type ([`Run.pm`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/lib/Test/Lintian/Run.pm) 438–442; [`Universal.pm`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/lib/Test/Lintian/Output/Universal.pm) `order`).

---

## 3. Finding all recipes that emit a given tag

### There is no tag→recipe index file

`t/COVERAGE` is a **2016-10-22** dump of untested tags ([`t/COVERAGE`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/t/COVERAGE) line 1). It is not an index of recipes and is stale relative to 1458 recipes.

### How lintian itself selects by tag

`private/runtests --onlyrun=tag:required-field` ([`t/recipes/README`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/t/recipes/README) 261–276; [`Filter.pm`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/lib/Test/Lintian/Filter.pm) 193–224):

1. Resolve the tag in the Profile (`required-field` → check `fields/required`, [`tags/r/required-field.tag`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/tags/r/required-field.tag) lines 1–3).
2. Select **every recipe whose `eval/desc` `Check:` lists that check**.

It does **not** grep `eval/hints`. So `tag:required-field` runs the whole `fields/required` recipe family, not “lines that mention that tag”.

`check:fields/required` is the same selection.

### Practical search for an importer

Grep `eval/hints` for the tag as the third field (not a substring). Example false positive: `doc-base-file-lacks-required-field` in [`t/recipes/checks/menus/legacy-binary/eval/hints`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/t/recipes/checks/menus/legacy-binary/eval/hints).

Word-boundary grep of `eval/hints` for `required-field` hits **four** files; the menus one is a different tag. The three real recipes are under `t/recipes/checks/fields/required/` (see the required-field section below).

### Do recipes test a whole check module?

Yes, by design: one `Check:` per recipe, `hints` may list **many tags** of that check. CONTRIBUTING: do not name tests after a single tag; “many tags need two or more tests” ([`CONTRIBUTING.md`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/CONTRIBUTING.md) 109–112). README (stale but still the intent): keep tests focused on a closely related set; `-general` tests may emit many tags ([`t/recipes/README`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/t/recipes/README) 288–360).

Largest `Check:` buckets in this tree include `debian/changelog` (72 recipes), `debhelper` (51), `debian/copyright/dep5` (50). The three `fields/required` recipes each emit **only** `required-field` (multiple lines / processable types).

---

## 4. Source-tree recipes vs recipes that must build `.deb` / `.dsc`

### Skeletons (the real “test type”)

`Type:`, `Sequence:`, and `Test-For:` are **not** how 2.139.0 classifies tests. `Sequence:` does not appear in recipes. `Type:` is almost always inherited from the skeleton (`native` vs `non-native`); only 7 `fill-values` files override `Type:`.

Skeleton files under [`t/skeletons/`](https://salsa.debian.org/lintian/lintian/-/tree/2.139.0/t/skeletons):

| Skeleton | Count | Build product | What lintian is run on |
| --- | ---: | --- | --- |
| `upload-native` | 917 | `.changes` (via `dpkg-buildpackage`) | Full upload: `.dsc` + `.deb` + `.changes` |
| `upload-non-native` | 350 | same, quilt/non-native | same |
| `source-native` | 85 | `.dsc` (`dpkg-source -b`) | Source package only |
| `source-non-native` | 38 | `.dsc` | Source package only |
| `deb` | 36 | `.deb` (hand-rolled `ar`) | Binary package only |
| `changes` | 21 | filled `test.changes` | `.changes` only |
| `upload-builder-only` | 11 | upload makefile without debian templates | special |

`Build-Product` comes from template fill-values, e.g.:

- upload: `[% $source %]_[% $no_epoch %]_[% $upload_type %].changes` ([`upload-make-builder.values`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/t/templates/upload-make-builder/fill-values.d/upload-make-builder.values))
- source: `[% $source %]_[% $version %].dsc` ([`source-make-builder.values`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/t/templates/source-make-builder/fill-values.d/source-make-builder.values))
- deb: `[% $source %].deb` ([`deb-make-builder.values`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/t/templates/deb-make-builder/fill-values.d/deb-make-builder.values))

The eval skeleton `testing` ([`t/skeletons/testing`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/t/skeletons/testing)) only supplies the TAP runner + lintian invocation templates, not a package.

Upload makefile copies `build-spec/debian/` onto `packagedir/debian` then runs `dpkg-buildpackage` ([`t/templates/upload-make-builder/Makefile.in`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/t/templates/upload-make-builder/Makefile.in) 46–55). Source makefile runs `dpkg-source -b` after the same copy ([`t/templates/source-make-builder/Makefile.in`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/t/templates/source-make-builder/Makefile.in) 60–77). Deb makefile assembles `DEBIAN/` + `root/` into a `.deb` ([`t/templates/deb-make-builder/Makefile.in`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/t/templates/deb-make-builder/Makefile.in)).

### Mapping to debmagic Subjects ([ADR 0016](../adr/0016-lint-subject-kinds.md))

- **Source tree**: the filled `packagedir` **before** `dpkg-buildpackage` / `dpkg-source` (debian/ overlay + orig + pre-build). Lintian itself does not run on that tree in these recipes; it runs on `.dsc`/`.deb`/`.changes`. Source-applicable **tags** are the `eval/hints` lines with `(source)` whose extra still makes sense on `debian/` (not `.dsc` basename extras).
- **Source package**: `source-*` skeletons and `(source)` hints that name a `.dsc`.
- **Binary package**: `deb` skeleton and `(binary)` hints.
- **`.changes`**: deferred in ADR 0016; `Skeleton: changes` recipes wait.

`eval/desc` fields that are **not** a test-type enum: `Testname`, `Check`, `Test-Against`, `Options` (extra CLI), `Profile` (default `debian`), `Match-Strategy` (`hints` | `literal`), `Output-Format`, `Exit-Status`, `Todo`, `Test-Architectures`, `Skeleton` (eval-side, default `testing`). Build-side `fill-values`: `Skeleton`, `Testname`, `Description`, `Version`, `Package-Architecture`, `Extra-Build-Depends`, `Test-Depends`, `Test-Conflicts` ([`t/recipes/README`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/t/recipes/README) 136–152; [`Hooks.pm`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/lib/Test/Lintian/Hooks.pm) 156–162).

Default lintian flags: `--pedantic --display-info --display-experimental --display-level +classification --show-overrides --check-part [% $check %]` ([`t/defaults/desc`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/t/defaults/desc) line 8). That is **not** the same as lintian’s default user profile; it is the test profile.

---

## 5. Reusing recipes as Source-tree fixtures without `Test::Lintian`

**Yes, for the oracle and for a materialized `debian/` tree.** Do not run `private/runtests` or `Test::Lintian::*`. ADR 0012 already says the first lint slice uses native tests (plain source tree + expected Tags), with an importer targeting that shape later.

### Double debian

Old: recipe `debian/` **was** the package’s `debian/` overlay, so `debian/rules` lived at `debian/debian/rules` ([WritingTests.pod](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/doc/tutorial/Lintian/Tutorial/WritingTests.pod) 158–163).

New: `build-spec/debian/` is that overlay (single `debian/` segment). Skeleton template set `debian (debian-native)` copies [`t/templates/debian-native/`](https://salsa.debian.org/lintian/lintian/-/tree/2.139.0/t/templates/debian-native) (`control.in`, `changelog.in`, `copyright`, `rules`, …) into the **work directory’s** `debian/`. Recipe files **replace** templates of the same name ([`Prepare.pm`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/lib/Test/Lintian/Prepare.pm) 176–177 `remove_surplus_templates`). Fill whitelist [`t/whitelists/debian-packaging`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/t/whitelists/debian-packaging) allows generating `control`, `changelog`, `rules`, `compat`, `source/format`, tests files from `.in`.

You never need a `debian/debian/` path in an imported fixture. If you copy a recipe raw, you must **flatten** `build-spec/debian/` → fixture `debian/` after template fill.

### What must be unpacked or generated

For an `upload-native` / `source-native` recipe to become a Source-tree fixture:

1. **Start from skeleton templates** (`t/templates/debian-native` plus builder templates) plus **recipe overlay** (`build-spec/debian/`, `build-spec/orig/`).
2. **Fill** `[% $source %]`, `[% $version %]`, `[% $standards_version %]`, `[% $author %]`, `[% $date %]`, `[% $description %]`, `[% $build_depends %]`, `[% $package_architecture %]`, `[% $dh_compat_level %]`. Defaults: author `Debian Lintian Maintainers <lintian-maint@debian.org>`, `Standards-Version` from env `POLICY_VERSION` at test time ([`Prepare.pm`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/lib/Test/Lintian/Prepare.pm) 226–229), `Dh-Compat-Level` from `DEFAULT_DEBHELPER_COMPAT` (231–234), `Source` defaults to `Testname` (211–212).
3. **Run `pre-build`** if present (path argument = package dir). Example: `generic-empty` deletes `debian/compat` and `debian/copyright`.
4. **Do not** need `dpkg-buildpackage` for Source-tree Subjects. You **do** need it (or `dpkg-source`) if you want `.dsc` extra lines or `.deb` files.
5. **Filter `eval/hints`** to `(source)` lines whose extra is about `debian/` (keep); drop `(binary)` / `(changes)` until those Subjects exist; drop `(source)` lines that only name a `.dsc` until Source-package Subject tests exist.
6. **Ignore** `eval/desc` `Check:` for native runs (debmagic selects Rules itself); use it only as a hint of which Rule family the recipe belongs to.

`Skeleton: deb` and `Skeleton: changes` are **not** source trees (`DEBIAN/control`, `test.changes.in`). Skip them for Source-tree import.

Literal/EWI recipes (`Match-Strategy: literal`) are lintian CLI tests, not tag oracles.

---

## 6. License

Same as lintian: **GPL-2+**.

- [`debian/copyright`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/debian/copyright) `Files: *` → `License: GPL-2+` (no `Files: t/` exception). Installed copy: `/usr/share/doc/lintian/copyright`.
- Repo root [`COPYING`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/COPYING) is GPL-2 text.
- Test harness modules (`Test::Lintian::*`, `private/runtests`) carry GPL-2+ headers.

Vendoring selected recipe trees into debmagic is a GPL-2+ copy; keep copyright notices and do not mix into a different license without a legal review. Expected `hints` lines are also from that tree.

---

## 7. Practical import path (pinning vs live lintian)

### `/usr/share/lintian` at test time — no

[`debian/lintian.install`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/debian/lintian.install) installs `bin`, `data`, `lib`, `profiles`, `tags`, `templates`, `vendors` — **not** `t/`. Confirmed: lintian 2.139.0 on this machine has no `/usr/share/lintian/t/`. Autopkgtest runs recipes from the **source** tree via `private/runtests` ([`debian/tests/build-and-evaluate-test-packages`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/debian/tests/build-and-evaluate-test-packages) lines 5–8). Ubuntu/Debian lintian drift is exactly why [ADR 0012](../adr/0012-parity-oracle-is-lintian-test-suite.md) forbids a live lintian binary as oracle.

### Git submodule of lintian — poor default

The submodule would be the whole project (checks, data, Perl harness, 1458 recipes). Debmagic’s SourceTreeTester must not depend on `Test::Lintian` or `dpkg-buildpackage` for Source-tree parity. A submodule still requires an importer to flatten skeletons; CI would need salsa availability.

### Copy selected recipe trees — better, still not native shape

Copying `t/recipes/checks/fields/required/generic-empty/` as-is is GPL-clean and pinable, but the fixture is **not** a Debian source tree until templates + fill + `pre-build` run. Debmagic would reimplement a slice of `Prepare.pm`.

### Recommended: vendor **imported** native fixtures, pin lintian **2.139.0**

Aligns with ADR 0012: “oracle is a pinned import of lintian’s own recipes — their expected `tags` files” (now `eval/hints`) “not live lintian.”

1. Pin comment/version: lintian **2.139.0** (`575a2bf1`).
2. One-shot (or checked-in script) importer, **not** in the SourceTreeTester hot path:
   - Input: salsa/git recipe + `t/templates` + `t/skeletons` + `t/defaults`.
   - Output: `tests/…/<id>/` with a **filled** `debian/` (and orig files if needed) and `expected.tags` derived from `eval/hints`.
3. Import **per tag / per Subject**, not the whole suite. For `required-field` Source-tree: only `generic-empty`; drop `.dsc` and `.deb` hint lines or split into later Subject tests.
4. Keep a pointer (`ORIGIN: lintian 2.139.0 path/to/recipe`) in the fixture so refreshes are grepable.
5. Do **not** update expected Tags from a distro `lintian` binary.

Refreshing the pin is a deliberate vendor bump (new tag, re-run importer, review hint diffs), same as any other vendored testdata.

---

## `required-field` recipes (concrete)

Tag definition: [`tags/r/required-field.tag`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/tags/r/required-field.tag) — `Check: fields/required`, severity error. Implementation: [`lib/Lintian/Check/Fields/Required.pm`](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/lib/Lintian/Check/Fields/Required.pm) — source `debian/control` needs Source/Maintainer/Standards-Version and each binary stanza Package/Architecture/Description (lines 38–40); `.dsc` extra fields (47–48); `.deb` Package/Version/Architecture/Maintainer/Description (42–44); `.changes` Format/Date/Source/Architecture/Version/Distribution/Maintainer/Changes/checksums/Files (52–53).

Three recipes, all `Check: fields/required`. Full `eval/desc` and `eval/hints` from disk:

### `generic-empty` — upload-native (best Source-tree candidate)

`eval/desc`:

```
Testname: generic-empty
Check: fields/required
```

`eval/hints`:

```
generic-empty (source): required-field generic-empty_1.0.dsc Standards-Version
generic-empty (source): required-field (in section for source) Standards-Version [debian/control:1]
generic-empty (source): required-field (in section for generic-empty) Description [debian/control:4]
generic-empty (binary): required-field generic-empty_1.0_all.deb Description
```

`build-spec/fill-values`:

```
Skeleton: upload-native
Testname: generic-empty
Package-Architecture: all
Description: Pathological empty package
```

Overlay `build-spec/debian/control.in` (replaces the skeleton control; **omits** Standards-Version and Description):

```
Source: [% $source %]
Maintainer: a <a@localhost.localdomain>

Package: [% $source %]
Architecture: [% $package_architecture %]
```

`pre-build` removes `debian/compat` and `debian/copyright`. `debian/rules` is a minimal `dpkg-gencontrol`/`dpkg --build` (not dh). After fill, Source-tree oracle is the **two `debian/control` lines**; the `.dsc` line needs a Source-package Subject; the `.deb` line needs Binary.

### `fields-general-missing` — binary only

`eval/desc`:

```
Testname: fields-general-missing
Check: fields/required
```

`eval/hints`:

```
fields-general-missing (binary): required-field fields-general-missing.deb Version
fields-general-missing (binary): required-field fields-general-missing.deb Package
fields-general-missing (binary): required-field fields-general-missing.deb Maintainer
fields-general-missing (binary): required-field fields-general-missing.deb Architecture
```

`Skeleton: deb`. `build-spec/DEBIAN/control.in` has Section/Priority/Depends/Description only — no Package/Version/Architecture/Maintainer. Wait for Binary Subject.

### `changes-missing-fields` — `.changes` only

`eval/desc`:

```
Testname: changes-missing-fields
Check: fields/required
```

`eval/hints`:

```
changes-missing-fields (changes): required-field test.changes Files
changes-missing-fields (changes): required-field test.changes Checksums-Sha256
changes-missing-fields (changes): required-field test.changes Checksums-Sha1
changes-missing-fields (changes): required-field test.changes Changes
```

`Skeleton: changes`. Deferred with `.changes` Subject.

---

## Sources

| What | Where |
| --- | --- |
| Current recipe authoring | [CONTRIBUTING.md](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/CONTRIBUTING.md) L104–167 |
| Stale recipe README | [t/recipes/README](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/t/recipes/README) |
| Stale `t/tests` tutorial | [WritingTests.pod](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/doc/tutorial/Lintian/Tutorial/WritingTests.pod) |
| Suite discovery / `tag:` filter | [lib/Test/Lintian/Filter.pm](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/lib/Test/Lintian/Filter.pm) |
| Hint line grammar | [lib/Test/Lintian/Output/Universal.pm](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/lib/Test/Lintian/Output/Universal.pm) L137–147 |
| Prepare / skeletons / double-debian reject | [lib/Test/Lintian/Prepare.pm](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/lib/Test/Lintian/Prepare.pm) |
| Run + compare hints | [lib/Test/Lintian/Run.pm](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/lib/Test/Lintian/Run.pm) |
| Defaults | [t/defaults/desc](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/t/defaults/desc), [fill-values](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/t/defaults/fill-values) |
| Install set (no `t/`) | [debian/lintian.install](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/debian/lintian.install) |
| License | [debian/copyright](https://salsa.debian.org/lintian/lintian/-/blob/2.139.0/debian/copyright) |
| Debmagic oracle ADR | [docs/adr/0012-parity-oracle-is-lintian-test-suite.md](../adr/0012-parity-oracle-is-lintian-test-suite.md) |
