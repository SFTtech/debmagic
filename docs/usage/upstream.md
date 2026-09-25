# Upstream versions

`debmagic upstream` replaces `uscan`/`uupdate`: it reads the package's existing `debian/watch` and `debian/copyright` and offers two operations — checking for new upstream versions and switching the package tree to one.

```shell
debmagic upstream list                 # newest upstream version newer than the changelog's
debmagic upstream list --all           # every candidate, newest first
debmagic upstream list --previous 3    # the 3 newest versions above the current one
debmagic upstream switch latest        # switch the tree to the newest version
debmagic upstream switch 3.12.0       # switch to a specific version
debmagic upstream switch latest --dry-run
```

## `upstream list`

Queries the sources declared in `debian/watch` and prints candidate upstream versions, sorted with Debian version semantics.
By default only versions newer than the changelog's current upstream version are shown (newest first); `--all` shows everything the watch file finds.

The changelog's version is normalized with the watch file's `Dversion-Mangle` rules before comparing, so `+dfsg` suffixes and similar don't hide available updates.

## `upstream switch`

Fetches the chosen version's tarball, applies the package's repack configuration, writes the `orig` tarball into the package's output dir (`build/` by default) and replaces the source tree's contents — everything except `debian/`, which is kept as-is.
The changelog is not touched; add the new version entry yourself (automated changelog handling is planned).

`--dry-run` reports what would happen — tarball URL, repack excludes, version suffix — without downloading anything.

For a concrete version, the tarball URL is resolved without scraping the download listing:

1. the distro archives (the `launchpad` orig method) — a version Debian or Ubuntu published needs no upstream download at all
2. the watch pattern, read as a URL template: `@ANY_VERSION@` becomes the version, `@ARCHIVE_EXT@` each archive extension in turn (the extension of the project's existing orig tarball is probed first), and the constructed URLs are probed — this also finds versions that have already fallen out of the listing
3. the listing, fetched as a *structure teacher*: any listed sibling reveals the prefix/extension shape, and the requested version's URL is constructed from it — this covers patterns pure inversion rejects (multiple capture groups, regex constructs) and compression changes
4. the listing, filtered for the requested version — the last resort

`upstream list` always scrapes the listing, since enumeration is its job.

### Signature verification

After downloading, the tarball's PGP signature is verified against the keyring in `debian/upstream/signing-key.asc` — the same location on every dpkg distribution, Ubuntu included (Ubuntu packages use the same `debian/` packaging layout; there is no Ubuntu-specific path).
Verification is on by default and uses the same OpenPGP backend as signing — `gpg` by default, `sq` (Sequoia) when `sign.tool = "sequoia"`, or your custom command when `sign.tool = "custom"`: `sign.verify_command` with `{file}`, `{signature}` and `{keyring}` placeholders (without `{signature}` the signature path is appended as the last argument), falling back to `sign.sign_command`.

The signature is looked up via the watch file's `Pgp-Sig-Url-Mangle` rules; without them the common suffixes (`.asc`, `.sig`, `.sign`, `.pgp`, `.gpg`) are tried next to the tarball.
If no signature exists upstream, the switch fails — disable the check per-run with `--no-signature-check` or persistently with `verify_signatures = false`:

```toml
[upstream]
verify_signatures = false
```

Export the upstream key with `gpg --export --armor <keyid> > debian/upstream/signing-key.asc`.

### Repacking

Two sources are merged:

- `Files-Excluded` in `debian/copyright` — the standard way to strip non-DFSG or useless files; `Files-Excluded-<component>` works for multiple-upstream-tarball packages
- the watch file's `Repack`/`Repacksuffix` options, e.g. `Repacksuffix: +dfsg` appends the suffix to the upstream version of the repacked tarball

The repacked `orig` tarball uses the canonical Debian layout: a single top-level `<source>_<version>/` directory, xz-compressed.

## Watch file support

`debian/watch` versions 2–5 are parsed (version 1 is rejected).
The common options work: `Source`, `Matching-Pattern` (including the `@ANY_VERSION@`/`@ARCHIVE_EXT@`/`@PACKAGE@` substitutions), `Search-Mode`, `Uversion-Mangle`, `Dversion-Mangle`, `Filename-Mangle`, `Download-Url-Mangle`, `Pgp-Mode`/`Pgp-Sig-Url-Mangle`, `Repack`, `Repacksuffix`, `Component` (for MUT packages), `Untrackable`.

http(s) sources are fetched directly; ftp sources fall back to `curl` (install it when a watch file needs it).

Rare options with no debmagic equivalent yet (`Mode: git`/`svn`, `Version-Schema: group/checksum`, `Page-Mangle`, `Update-Script`, ...) produce a clear error suggesting an explicit upstream declaration instead of silently misbehaving.

## Orig tarballs in builds

`debmagic build source` needs the `orig` tarball for `3.0 (quilt)` packages and finds it without any configuration:

1. the package's output dir (`build/` by default) — where `upstream switch` puts it
2. next to the source tree (`../`), the conventional location — used as-is, never written to
3. the download cache (`~/.cache/debmagic/orig/<name>/<upstream_version>/`) — populated by the `launchpad` method; a cached tarball is hardlinked into the output dir when both are on the same filesystem, copied otherwise
4. the configured fetch method — `launchpad` by default, no configuration needed:

```toml
[orig_tarball]
method = "launchpad"   # the default: the distro archives via Launchpad's download URLs
# or:
method = "debian"        # Debian's own archive pool, without Launchpad
# method = "ubuntu"     # Ubuntu's own archive pool, without Launchpad
# mirrors default to the distro archives; any apt-style mirror root works:
# debian_mirror = "https://mirror.example.com/debian"
# ubuntu_mirror = "http://mirror.example.com/ubuntu"
# or:
method = "custom"        # requires 'command'
command = "uscan --download --download-current-version && cp ../foo*.orig.tar.* {output_dir}/"
# or:
method = "disabled"      # never fetch; build only with tarballs found locally
```

The `launchpad` method constructs the orig tarball's download URL directly (`launchpad.net/<distro>/+archive/primary/+files/<name>_<upstream>.orig.tar.<ext>`) and probes the compression extensions, searching Ubuntu first, then Debian (which Launchpad mirrors). No API query, configuration or authentication is needed — and no published version of the changelog head either: the orig tarball only depends on the upstream part, which older publications share.

The custom command runs via `sh -c` in the source dir with `{name}`, `{version}`, `{upstream_version}`, `{source_dir}` and `{output_dir}` placeholders, and `DEBMAGIC_ORIG_OUTPUT_DIR` set.

`3.0 (native)` packages skip all of this — they have no `orig` tarball.

## Including the orig in uploads

Whether an upload carries the `orig` tarball is decided at upload time, not build time:

```shell
debmagic build source --include-orig=auto   # default
debmagic upload ppa:you/your-ppa --include-orig=yes
```

- `auto` (default): include when the target hasn't seen this upstream version yet — no upload record, a different recorded version, or a deltarebase onto a Debian version Ubuntu never had (detected from the changelog: current revision has an `ubuntu` component, the previous entry's doesn't)
- `yes`/`no`: always/never

On `upload`, a decision that disagrees with the `.changes` file rewrites its file listing (adding or removing the `orig` entry with correct checksums) and re-signs it.
Before adding, the tarball is verified against the checksum the `.dsc` recorded, so a stale orig next to the `.changes` fails loudly instead of being rejected by the archive.