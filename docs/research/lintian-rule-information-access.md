# What lintian rules need to inspect

Primary source: Debian lintian **2.139.0** as installed (`/usr/share/lintian/`). Upstream git: [salsa.debian.org/lintian/lintian](https://salsa.debian.org/lintian/lintian). Tag catalog: 1529 `.tag` files (658 warning, 563 error, 166 info, 103 pedantic, 39 classification). Check modules: 343 under `lib/Lintian/Check/`. Classification tags are not Rules (ADR 0022) and are ignored below.

Lintian does not run a Rule against “the package” as a blob. Each Check module is a Moo role on `Lintian::Check`; `run` walks processable-specific **indexes** and then calls type hooks (`source`, `installable`/`binary`, `always`). Source walks `orig` then `patched` file indexes; binary/udeb walk `control` then `installed` ([`lib/Lintian/Check.pm`](https://salsa.debian.org/lintian/lintian/-/blob/master/lib/Lintian/Check.pm) `run` / `visit_files`).

Processable types lintian knows: `source`, `binary`, `udeb`, `changes`, `buildinfo` ([`lib/Lintian/Group.pm`](https://salsa.debian.org/lintian/lintian/-/blob/master/lib/Lintian/Group.pm) `%SUPPORTED_TYPES`). Debmagic Subjects today are Source tree, Binary package, Source package — not `.changes` / `.buildinfo`.

---

## Taxonomy of information kinds

Rough size is **tag count by Check: prefix** (not module count). `debian/*` is 373 tags because it is a bag of distinct debian/ files, not one kind.

### Large (must be represented in the first native slice)

| Kind | What is read | Where in lintian | Scale |
| --- | --- | --- | --- |
| **Deb822 control fields** | `debian/control`, `.dsc` fields, `.deb` `control` stanza: field names, values, source vs installable paragraphs | `Fields/*`, `Debian/Control/*`, [`Lintian::Debian::Control`](https://salsa.debian.org/lintian/lintian/-/blob/master/lib/Lintian/Debian/Control.pm), [`Lintian::Deb822`](https://salsa.debian.org/lintian/lintian/-/blob/master/lib/Lintian/Deb822.pm) | **215** tags under `fields/*` |
| **Patched / orig source file index** | Every path in the unpacked source (after quilt): name, type, mode, symlink target, `file(1)` magic, sometimes bytes | `visit_patched_files` / `visit_orig_files`; [`Lintian::Index::Item`](https://salsa.debian.org/lintian/lintian/-/blob/master/lib/Lintian/Index/Item.pm), [`Index::FileTypes`](https://salsa.debian.org/lintian/lintian/-/blob/master/lib/Lintian/Index/FileTypes.pm) (`file --raw`); `Files/*`, `Cruft` | **202** `files/*` + **16** `cruft` |
| **debian/changelog** | Debian changelog grammar: entries, versions, distributions, dates, Closes, maintainer | [`Debian/Changelog.pm`](https://salsa.debian.org/lintian/lintian/-/blob/master/lib/Lintian/Check/Debian/Changelog.pm), [`Lintian::Changelog`](https://salsa.debian.org/lintian/lintian/-/blob/master/lib/Lintian/Changelog.pm) | **52** tags |
| **debian/copyright** (plain + DEP-5) | Presence of `debian/copyright` / `/usr/share/doc/*/copyright`; DEP-5 Deb822 (Format, Files, License paragraphs); license text size | [`Debian/Copyright.pm`](https://salsa.debian.org/lintian/lintian/-/blob/master/lib/Lintian/Check/Debian/Copyright.pm), [`Debian/Copyright/Dep5.pm`](https://salsa.debian.org/lintian/lintian/-/blob/master/lib/Lintian/Check/Debian/Copyright/Dep5.pm) | **36** + **41** |
| **debian/rules** | Shebang (`/usr/bin/make -f`), executable bit, makefile targets, `include`, dh sequencer lines | [`Debian/Rules.pm`](https://salsa.debian.org/lintian/lintian/-/blob/master/lib/Lintian/Check/Debian/Rules.pm), `data/rules/` | **36** |
| **Package relations** | Depends/Recommends/… and Build-Depends parsed as Policy 7.1 alternatives | [`Fields/PackageRelations.pm`](https://salsa.debian.org/lintian/lintian/-/blob/master/lib/Lintian/Check/Fields/PackageRelations.pm), `Lintian::Relation`; also Debhelper/Python cross-checks | **62** (`fields/package-relations` alone) |
| **Installed binary file index** | Paths inside the `.deb` data.tar: mode, owner, size, magic, duplicates, FHS | `visit_installed_files`; `Files/Hierarchy/*`, `Files/Permissions`, `Usrmerge`, … | bulk of `files/*` on binary |
| **Scripts / shebang** | `#!` interpreter, executable bit, interpreter → package map | [`Scripts.pm`](https://salsa.debian.org/lintian/lintian/-/blob/master/lib/Lintian/Check/Scripts.pm), `data/scripts/interpreters` | **69** (largest single Check:) |

### Medium

| Kind | What is read | Scale / notes |
| --- | --- | --- |
| **ELF / shared libraries** | `file` “ELF … not stripped”; objdump/readelf: NEEDED, RPATH, SONAME, hardening notes, symbols | `Index::Elf`, `Binaries/*` (**33**), `Libraries/*` (**23**) |
| **Debhelper** | `debian/rules` dh usage, compat level (`debian/compat` or `debhelper-compat (= N)`), addons | `Debhelper.pm` + `data/debhelper/` (**42**) |
| **Maintainer scripts** | `DEBIAN/{pre,post}{inst,rm}` text (control.tar) | `MaintainerScripts/*` (**17**), plus `visit_control_files` in Scripts/InitD/Apache2 |
| **Init / systemd / udev / cron / apache** | Unit files, init.d LSB headers, tmpfiles, udev rules | systemd **23**, init-d **40**, udev **4**, apache2 **14** |
| **Manpages / doc / desktop / menus** | Installed documentation layout, `.desktop`, menu-format | documentation **51**, menu-format **45**, menus **41**, desktop **25** |
| **Language ecosystems** | Python/Java/JS/Perl/… bytecode, paths, team conventions | `Languages/*` **104** |
| **Patches / source format / watch** | `debian/source/format`, quilt series, DEP-3, `debian/watch` | source-dir, patches, watch (~40 combined) |
| **Static `data/` tables** | Policy releases, obsolete packages, interpreters, spelling, archive sections, fonts | `/usr/share/lintian/data/` — not package content; many tags are “field value ∈ table” |

### Smaller / later

- **`.changes` / `.buildinfo` / group** — sibling packages in one upload (`GroupChecks`, `ChangesFile`). Debmagic has no matching Subject yet.
- **`.dsc` checksums and Files list** — Source package identity (`Processable::Fields::Files`).
- **shlibs / symbols / triggers / md5sums / conffiles** — binary control.tar policy files.
- **Spelling** — English dictionaries over descriptions and binaries.
- **Privacy / privacy-breach URLs, obsolete sites** — content scan + URL lists.
- **Java bytecode, fonts, images, orig tarball vs patched diffstat**.

Indexes lintian attaches to file items (the “collector” layer): file types (`file(1)`), ELF, Java, ar, md5sums, strings (`lib/Lintian/Index/*.pm`). Native Rules will eventually need the same facts, even if they start as in-rule `std::fs` + `file`/`readelf`.

---

## Five starter tags

Chosen to force five **different I/O surfaces**, default-visible severity (error or warning, not experimental), no archive-wide dump, and a path we can implement on current Subjects. Together they stand in for Deb822 fields, changelog, debian/rules, the source file index + magic, and the binary ELF/installed-file index.

### 1. `required-field`

- **Severity:** error. **Check:** `fields/required`. **Renamed-from** includes `no-standards-version-field`, `no-maintainer-field`, …
- **Subject:** Source tree (`debian/control` paragraphs), Source package (`.dsc` fields), Binary package (installation control).
- **Inspects:** Deb822: whether Policy-required fields exist. Source `debian/control` needs `Source`, `Maintainer`, `Standards-Version`; each installable paragraph needs `Package`, `Architecture`, `Description`. `.dsc` adds Format/Version/Checksums/Files. `.deb` needs Package/Version/Architecture/Maintainer/Description ([`Fields/Required.pm`](https://salsa.debian.org/lintian/lintian/-/blob/master/lib/Lintian/Check/Fields/Required.pm), Policy 5.2–5.4).
- **Cluster:** all **215** `fields/*` tags share this control object.
- **Native sketch:** parse Deb822 into source + named binary stanzas; emit one Diagnostic per missing required name. Extra/context is the field name (and “in section for …” on `debian/control`).

### 2. `syntax-error-in-debian-changelog`

- **Severity:** warning. **Check:** `debian/changelog`.
- **Subject:** Source tree (and Source package via patched tree); also fires from the binary changelog copy when that lint runs on `.deb`.
- **Inspects:** Debian changelog grammar. Parser errors are `[$line, $message]` on `Lintian::Changelog->errors`, then hinted with the quoted condition ([`Debian/Changelog.pm`](https://salsa.debian.org/lintian/lintian/-/blob/master/lib/Lintian/Check/Debian/Changelog.pm) around the `for my $error (@{$changelog->errors})` loop).
- **Cluster:** **52** changelog tags (versions, NMU, distributions, dates, Closes, line length).
- **Native sketch:** parse `debian/changelog`; each parser error is a Diagnostic at that line. Do not need archive or `data/` for this tag.

### 3. `debian-rules-missing-required-target`

- **Severity:** error. **Check:** `debian/rules`. Policy 4.9.
- **Subject:** Source tree.
- **Inspects:** `debian/rules` as a makefile: required targets `build`, `build-arch`, `build-indep`, `binary`, `binary-arch`, `binary-indep`, `clean`. Implementation scans lines for target names, `include` of known helper makefiles (`data/rules/known-makefiles`), and treats a `dh` sequencer as providing all of them ([`Debian/Rules.pm`](https://salsa.debian.org/lintian/lintian/-/blob/master/lib/Lintian/Check/Debian/Rules.pm) `%TAG_FOR_POLICY_TARGET`).
- **Cluster:** **36** `debian/rules` tags plus Debhelper’s use of the same file.
- **Native sketch:** locate `debian/rules` in the Source tree (follow symlink); collect declared targets (and `dh` / known includes); hint each missing policy target as extra. A first slice can special-case `dh` and ignore exotic includes (parity holes documented until filled).

### 4. `source-contains-prebuilt-windows-binary`

- **Severity:** warning. **Check:** `files/source-missing`.
- **Subject:** Source tree / Source package (patched index). Skip `.pc/` (lintian already excludes quilt metadata in `visit_files`).
- **Inspects:** every regular file’s `file(1)` type string for PE32/PE64 / MS-DOS/COM executable ([`Files/SourceMissing.pm`](https://salsa.debian.org/lintian/lintian/-/blob/master/lib/Lintian/Check/Files/SourceMissing.pm) `visit_patched_files`).
- **Cluster:** **202** `files/*` tags that need a source file index + magic (prebuilt objects, missing sources, names, encoding).
- **Native sketch:** walk the tree; run `file` (or a magic crate) per file; match the same PE/MS-DOS regex. No `data/` file required.

### 5. `unstripped-binary-or-object`

- **Severity:** error. **Check:** `binaries/debug-symbols`. Policy 10.1 / 10.2.
- **Subject:** Binary package.
- **Inspects:** installed files whose `file` type contains `ELF` and `not stripped`, with exclusions: `*.o`/`*.ko`, `*-dbg` packages, `lib/debug/`, Guile `.go`, OCaml `.gox`, and Caml bytecode executables ([`Binaries/DebugSymbols.pm`](https://salsa.debian.org/lintian/lintian/-/blob/master/lib/Lintian/Check/Binaries/DebugSymbols.pm)).
- **Cluster:** **33** `binaries/*` + **23** `libraries/*` that need unpack + ELF/magic (hardening, RPATH, NEEDED, SONAME).
- **Native sketch:** unpack `.deb` data.tar; for each file, `file` (and later readelf); emit on unstripped ELF with the same path exclusions.

---

## Rejected for this slice

| Tag | Why not |
| --- | --- |
| `syntax-error-in-dep5-copyright` | Strong cluster (77 copyright tags) but same I/O family as Deb822 (`required-field`). Next after the five. |
| `bad-relation` / `depends-on-obsolete-package` | Relation grammar is important (**62** tags) but needs Policy 7.1 parsing; obsolete variant needs `data/` package lists. Build on `required-field`’s control object. |
| `missing-dep-for-interpreter` | Covers scripts (**69**) *and* Depends, but needs `data/scripts/interpreters` and a `.deb`. Prefer ELF as the first binary pipeline; scripts next. |
| `script-not-executable` | Warning, good, but thinner than ELF for unlocking Binaries/Libraries. |
| `no-copyright-file` | Binary-only path presence (`/usr/share/doc/<pkg>/copyright`); too little parser surface. |
| `out-of-date-standards-version` | Info — not default Selection (ADR 0010). Needs policy-release dates vs changelog timestamp. |
| `debian-rules-not-executable` | Pedantic. |
| `debian-watch-file-is-missing` / `missing-debian-source-format` | File existence only. |
| `source-contains-prebuilt-binary` | Pedantic; Windows-PE sibling is warning and enough to force magic. |
| `hardening-no-pie` | Needs full ELF notes / hardening collector; `unstripped-*` is the thinner ELF on-ramp. |
| `invalid-standards-version` | Error, but a single field format check — subset of Deb822, less leverage than `required-field`. |
| Group / `.changes` tags | No Subject kind yet (ADR 0018). |
| Language-specific (Java bytecode, Python `.pyc`, …) | Niche collectors; `source-contains-prebuilt-windows-binary` already forces magic. |
| `package-installs-python-bytecode` | Error, but language-specific installed-file name check. |

---

## Suggested order after these five

1. `syntax-error-in-dep5-copyright` — second Deb822 dialect (copyright-format 1.0).
2. `bad-relation` — Policy relation parser on the control object from (1).
3. `missing-dep-for-interpreter` — installed scripts + interpreter table + Depends.
4. `missing-debian-source-format` or a quilt/DEP-3 tag — `debian/source/` and patches.
5. A Debhelper tag (`compat` / `dh` addon) — `debian/rules` plus `data/debhelper/`.
