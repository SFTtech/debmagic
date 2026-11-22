# lintian Check API → debmagic lint

How the lintian 2.144.0 Check API that most Checks use maps onto debmagic lint. Read it next to the Check module you are porting. `just lintian-tag <tag>` prints the rows the Check actually uses. `just lint-api` lists the debmagic side in full.

The first column is matched against Check sources by `scripts/lintian_tag.py`. Keep one lintian API name in backticks per row, written as it appears in Perl.

## Check hooks

| lintian | debmagic | Notes |
| --- | --- | --- |
| `sub source` | `impl SourceRule` → `run(&self, ctx: &mut SourceContext)` | One trait covers Source tree and Source package (ADR 0040). |
| `sub installable` | `impl BinaryPackageRule` → `run(&self, ctx: &mut BinaryPackageContext)` | Covers deb, udeb, and ddeb (ADR 0023). |
| `sub binary` | `impl BinaryPackageRule`, return early when `ctx.is_udeb()` | |
| `sub udeb` | `impl BinaryPackageRule`, only when `ctx.is_udeb()` | |
| `sub always` | implement both traits | |
| `sub changes` | `impl ChangesRule` → `run(&self, ctx: &mut ChangesContext)` | The `.changes` of a Group (ADR 0055). `ctx.changes()` is the parsed file, `ctx.changes().files()` its `Files` and `Checksums-*` entries (lintian's `files` hash); `ctx.sibling(name)` is a file next to it. Hints get processable type `changes`. |
| `sub buildinfo` | `impl BuildinfoRule` → `run(&self, ctx: &mut BuildinfoContext)` | The `.buildinfo` of a Group; `ctx.buildinfo()`. Hints get processable type `buildinfo`. |
| `visit_installed_files` | iterate `ctx.data_files()?.entries()` or `.query()` | Entries are sorted by path, like `sorted_list`. |
| `visit_control_files` | iterate `ctx.control_files()?` | Maintainer scripts and `control`, `md5sums`, … (ADR 0045). |
| `visit_patched_files` | iterate `ctx.files()?`, **skip paths under `.pc/`** | lintian drops `.pc/` in `visit_files`; `files()` does not. |
| `visit_orig_files` | iterate `ctx.orig_files()?` | `None` on a Source tree; empty when Native. |
| `$self->group` | `impl GroupRule` → `run(&self, ctx: &mut GroupContext)` | Runs once per Group that has a `.dsc`, as lintian runs a Check's `source` hook on the Group's source processable; hints attach to the Source package (ADR 0055). `ctx.changes()`, `ctx.dsc()`, `ctx.buildinfo()`, `ctx.installables()`. |
| `$group->get_installables` | `ctx.installables()` | `GroupInstallable`: `name()`, `version()`, `architecture()`, `multi_arch()`, `provides()`, `is_udeb()`, `fields()`, `relation(field)`, `strong_relation()`. Debs and ddebs before udebs. |
| `$group->direct_dependencies` | `ctx.direct_dependencies(&installable)` | Strong dependencies (Pre-Depends, Depends) on sibling installables; self-dependencies are left out. |
| `$installable->installed` (of a sibling) | `ctx.data_files(&installable)?` | The sibling's Data files File tree, shared with Binary-package Rules; `ctx.control_files(&installable)` for its Control files. |
| `$group->source` (its control and files) | `ctx.debian_control()`, `ctx.patched_files()`, `ctx.orig_files()` | From one `dpkg-source` extract shared with Source-package Rules. `None` when the Group has no `.dsc`. |

## Hints

| lintian | debmagic | Notes |
| --- | --- | --- |
| `pointed_hint` | `ctx.diagnostic(Diagnostic::error(extra).with_location(Location { path, line, column: None }))` | `Diagnostic::warn` for warning Tags. Severity follows the `.tag` file. |
| `hint` | `ctx.diagnostic(Diagnostic::error(extra))` | No Location; the Subject is the location. |
| `$item->pointer` | `Location { path: entry.path().to_path_buf(), line, column: None }` | The `[pointer]` is stripped from `eval/hints` extras, except when the extra is empty. |
| `$self->spelling_tag_emitter` | none yet | |

Extras keep lintian's words and order. Recipes are the oracle.

## File items

| lintian | debmagic | Notes |
| --- | --- | --- |
| `$item->name` | `entry.path()` | Relative, no leading `/`. lintian adds a trailing `/` to directories, debmagic does not. Paths keep raw bytes; use `as_os_str().as_bytes()` for non-UTF-8 names. |
| `$item->basename` | `entry.path().file_name()` | For a string, `interpreter::basename` mirrors Perl's `basename`. |
| `$item->dirname` | `entry.path().parent()` | |
| `$item->is_file` | `entry.is_file()` | lintian's `is_file` includes hardlinks. A fifo or device is not a file. |
| `$item->is_regular_file` | `entry.is_regular_file()` | |
| `$item->is_dir` | `entry.is_dir()` | |
| `$item->is_symlink` | `entry.is_symlink()` | |
| `$item->is_hardlink` | `entry.is_hardlink()` | |
| `$item->perm =~ /^[pcb]/` (fifo, character, block device) | `entry.kind()` is `EntryKind::Fifo`, `CharDevice` or `BlockDevice` | No content, no File type, device numbers not kept. A `match` on `EntryKind` must list them; `.kinds([...])` queries are unaffected. |
| `$item->link` | `entry.link_target()` (symlink) or `entry.hardlink_target()` (hardlink) | As stored, not resolved. |
| `$item->link_normalized` | none shared | Resolve with `Path` logic. Several Rules have private resolvers, for example `testsuite/sections.rs::resolve`. |
| `$item->resolve_path` | `tree.get(path)`, then follow `link_target()` yourself | |
| `$item->is_open_ok` | regular file or hardlink, or a symlink that resolves to one inside the tree | No shared helper; `SourceContext::quilt_series` has a private `follow_symlinks`. |
| `$item->unpacked_path` | `tree.read(entry)?` | Bytes, not a host path. |
| `$item->bytes` | `tree.read(entry)?` | |
| `$item->decoded_utf8` | `std::str::from_utf8(&tree.read(entry)?)` | lintian yields empty on invalid UTF-8; match that. |
| `$item->size` | `tree.read(entry)?.len()` | No size field yet. |
| `$item->operm` | `entry.mode()` | Permission bits `0o7777` (ADR 0046). |
| `$item->octal_permissions` | `format!("{:04o}", entry.mode())` | |
| `$item->is_executable` | `entry.is_executable_file()` (file or hardlink), or `entry.mode() & 0o111 != 0` | |
| `$item->is_setuid` | `entry.mode() & 0o4000 != 0` | |
| `$item->is_setgid` | `entry.mode() & 0o2000 != 0` | |
| `$item->identity` | `entry.has_identity("root/games")`, or `entry.owner()?.identity()` | `owner()` is `None` on host File trees and fixtures that set none; it is not `root/root`. Binary package Entries have it from the tar header. |
| `$item->owner` | `entry.owner()?.user()` | The name, else the number (`0` is `root`). |
| `$item->group` | `entry.owner()?.group()` | As `owner`. |
| `$item->uid` | `entry.owner()?.uid()` | |
| `$item->gid` | `entry.owner()?.gid()` | |
| `$item->file_type` | `entry.file_type()` (ELF, PE, DOS MZ only), `ElfFacts`, `SfntMagic`, or a sniff in the Rule | No `file(1)` strings (ADR 0041). Match the lintian regex against the facts it implies. |
| `$item->is_elf` | `entry.file_type()` is `FileType::Elf`, or `ElfFacts::from_bytes` is `Some` | `ElfFacts` also covers ELF files whose section headers are cut off (ADR 0047). |
| `$item->elf` | `ctx.data_elf_files()?.get(path)` → `ElfFacts` | `needed()`, `soname()`, `rpath()`, `runpath()`, `interpreter()`. No symbols or hardening yet. |
| `$item->hashbang` | `Hashbang::read(&bytes)` | `interpreter.rs`. |
| `$item->interpreter` | `Hashbang::read(&bytes)?.interpreter()` | Look it up with `interpreter::lookup(basename)`. |
| `$item->is_script` | `Hashbang::read(&bytes).is_some()` | |
| `$item->calls_env` | `Hashbang::calls_env()` | |
| `$item->is_shell_script` | interpreter basename is `sh`, `bash`, `dash`, … | |
| `$item->is_maintainer_script` | Control file named `preinst`, `postinst`, `prerm`, `postrm`, or `config` | `maintainer_script.rs` holds the line walk. |
| `$item->strings` | none | |
| `$item->java_info` | none | |
| `$item->md5sum` | none | |

## Processable

| lintian | debmagic | Notes |
| --- | --- | --- |
| `processable->name` | `ctx.package_name()` (Binary package); `debian_control()?.source().get("Source")` (source) | |
| `processable->source_name` | `ctx.source_name()` (Binary package) | |
| `processable->type` | `ctx.is_udeb()`; `ctx.kind()` on source | |
| `processable->fields` | `ctx.installation_control().fields()`, `ctx.dsc()?.fields()`, `ctx.debian_control()?.source()` | All are `ControlParagraph`: `get`, `declares`, `names`, `unfolded_value`, `untrimmed_value`. |
| `fields->value` | `paragraph.get(name)` | |
| `fields->unfolded_value` | `paragraph.unfolded_value(name)` | |
| `fields->declares` | `paragraph.declares(name)` | |
| `processable->debian_control` | `ctx.debian_control()` | `source()` and `installables()`. `None` on a syntax error. |
| `debian_control->installables` | `ctx.debian_control()?.installables()` | `Installable::name()`, `fields()`, `package_type()`. |
| `processable->relation` | `paragraph.relation(field)` or `paragraph.relations_and(&[...])` | `relation('all')` on a binary is `relations_and(&["Pre-Depends", "Depends", "Recommends", "Suggests"])`; `Build-Depends-All` is the three `Build-*` fields on the source paragraph. |
| `processable->binary_relation` | `installable.fields().relation(field)` | |
| `relation_norestriction` | `PackageRelation::restriction_less()` | |
| `satisfies` | `PackageRelation::satisfies(condition)` | Condition is lintian relation text. |
| `processable->native` | `fields_version::is_native(ctx)` | `pub(super)`: lift it to a shared module before using it elsewhere. Private copies also exist in `changelog_doc.rs` and `debian_watch/file.rs`. |
| `processable->changelog` | `ctx.debian_changelog()` | `entries()`, `errors()`. Installed changelogs on a Binary package are parsed from bytes; see `debian_changelog/installed.rs`. |
| `processable->patched` | `ctx.files()` | |
| `processable->installed` | `ctx.data_files()` | `lookup` → `get`, `sorted_list` → `entries()`. |
| `processable->control` | `ctx.control_files()` | |
| `processable->orig` | `ctx.orig_files()` | |
| `processable->compat_level` | `ctx.debhelper()?.level()` | `DebhelperFacts` also has `commands()` and `addons()` (ADR 0049). |
| `processable->overrides` | `ctx.lintian_overrides()` | Parsed, never applied (ADR 0051). |
| `processable->is_non_free` | Section starts with `non-free` (or `non-free-firmware`, `contrib`, as the Check says) | |
| `processable->is_transitional` | Description starts with `transitional` | |

## Other

| lintian | debmagic | Notes |
| --- | --- | --- |
| `$self->data` | vendored file under `packages/debmagic/src/lint/data/` read with `include_str!` and `data::lines` | Vendor from the pinned clone, never `/usr/share/lintian`. |
| `$self->profile` | `lintian_tags::current_tag_name` | Tag catalogue only (ADR 0051). |
| `added_diversions` | `ctx.diversions()?.added()` | ADR 0053. |
| `Lintian::Relation` | `PackageRelation` | |
| `Lintian::Deb822` | `ControlParagraph`, `DebianControl::parse`, `Dsc::parse` | |
| `quilt` | `ctx.quilt_series()` | `patches()` and `mentions()` (ADR 0050). |
| `Font::TTF` | `sfnt::Sfnt`, `sfnt::SfntMagic` | ADR 0052. |

## Vendored iso-codes tables

`files/locales` reads lintian's `/usr/share/iso-codes` data at run time. debmagic vendors the derived tables under `packages/debmagic/src/lint/data/files/` instead, so results do not depend on the host:

| File | Content | lintian |
| --- | --- | --- |
| `locale-codes` | alpha_2 and alpha_3 of every ISO 639-3 entry, except reserved and special (`S`) types | `LOCALE_CODES` keys |
| `locale-group-codes` | alpha_3 of every ISO 639-5 entry | `LOCALE_GROUP_CODES` keys |
| `locale-preferred-codes` | `alpha_3 alpha_2` for each code with a two-letter code | `LOCALE_CODES` values (`incorrect-locale-code` fix) |

The confusing language and country pairs (`al`→`sq`, `en_UK`→`en_GB`, ...) are lintian constants, kept in `rules/lintian/files_locales/locale_folder.rs`.

Regenerate with `scripts/vendor_iso_codes.py` (stdlib only). It reads `iso_639-3.json` and `iso_639-5.json` from `--json-dir` (default `/usr/share/iso-codes/json`) and writes the three files to `--out-dir`. `--version` is recorded in each table header; by default it is the upstream part of the installed `iso-codes` package (epoch and Debian revision removed). The vendored tables are pinned to iso-codes **4.20.1**, so regenerate with `--version 4.20.1` on a host with another version only if the pin is moved on purpose; the result must then be reviewed as a data change. Run `uv run pytest tests/test_vendor_iso_codes.py` after changing the script.
