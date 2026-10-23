# Building packages

Quick reference to build a Debian/Ubuntu package with `debmagic`.


## TL;DR

- Entry point: `debmagic build binary` — build on *any* `debian/`-packaged source tree (including packages with [`debian/rules.py`](packaging.md))
- Source package: [`debmagic build source`](source.md), creates a `.dsc` without compilation

```shell
cd your-package
debmagic build binary --driver lxd \
  --output-dir /tmp/build-artifacts
```

## Available options

`debmagic build`:

| Option | Description |
|---|---|
| `--driver <...>` | [Build environment driver to use](#picking-a-driver) |
| `--source-sync <mode>` | [Which source files to include](#source-file-staging) |
| `--persistent <mode>` | How long the build environment outlives this build: `on-failure` (default) keeps it when the build command fails, `always` keeps it after every build, `no` tears it down. A bare `--persistent` means `always` |
| `--incremental` | Do incremental builds by syncing changed sources only; forces `--persistent=always` |
| `--distro <name>` | [Select the target distro/release](#selecting-a-distrorelease) (e.g. `trixie`, `resolute`) |
| `--proposed` | Use [build dependencies from `proposed`](#proposed-dependencies) pocket |
| `--sign` | [GPG-sign the resulting `.changes`/`.dsc`/`.buildinfo`](#signing) |
| `--clean` | Run [`debian/rules clean` before building](#cleaning) |
| `--debug-symbols` | [Build the automatic `-dbgsym` debug symbol packages](#building-debug-symbol-packages) |
| `--test` | [Run the package's test suite](#running-tests) |
| `--notify` | [Desktop notification and terminal bell](#notifications) when the build finishes or signing needs a key touch |
| `--apt-mirror <url>` | [Mirror URL](#mirror-selection) |
| `--apt-update-age <when>` | [When a persistent environment re-runs `apt-get update` and `upgrade`](#apt-update-age) |
| `--source-dir <dir>` | Directory containing the `debian/` package directory |
| `--output-dir <dir>` | Directory to put the resulting build artifacts |

[`debmagic env shell`](#inspecting-a-failed-build) — attach an interactive shell to an Environment

## Picking a driver

Check what's installed and use the first that applies, in this order:

| Driver | Check it's available | Isolation |
|---|---|---|
| `lxd` / `incus` | `lxc list` / `incus list` | Full container isolation |
| `docker` | `docker info` | Full container isolation |
| `bare` | none (no daemon) | None — build-deps install with `sudo apt-get` directly on the host; only use in a disposable/CI environment |

There's no auto-detection; pick one and pass it explicitly every time (or configure it in a [`debmagic.toml`](config.md) file).

## Inspecting a failed build

When the build command itself fails and this run's Persistence is `on-failure` (the default) or `always`, the Environment stays and debmagic prints `debmagic env shell <id>`. Setup failures, and failures after the build command succeeded (export, signing), tear it down. `no` always tears it down and prints no hint.

```shell
debmagic env shell <id>
```

Each `debmagic env shell` is its own shell, so you can open more than one. A bare `debmagic env shell` attaches only when this checkout has exactly one Environment.


## Mirror selection

Fresh containers install their base tooling plus every `Build-Depends`, so slow mirrors directly translate into slow builds.
Pass `--apt-mirror` to replace the image's apt sources with a different mirror:

```shell
debmagic build binary --driver lxd --apt-mirror http://<mirror-host>/ubuntu ...
```

You can persistently set this flag in  `.config/debmagic/config.toml`.

## Apt update age

`--apt-update-age` decides when `apt-get update/upgrade` run again, measured from the last upgrade:

| Value | Behavior |
|---|---|
| `now` | Upgrade before every build |
| `never` | Only the initial upgrade on creation |
| `1d` (default), `12h`, `30m`, … | Upgrade again once the last one is older than this |

The bare driver builds on the host and never upgrades it.

The default `1d` matches a typical developer machine's daily apt refresh: repeated builds stay fast, while the index can't go arbitrarily stale.
Persist the setting as `apt_update_age = "1d"` in [`debmagic.toml`](config.md).

## Source file staging

Before building, `debmagic` stages the source tree into the build environment.
`--source-sync <mode>` controls which files are staged, so you always know what ends up in the build and in a generated source package:

| Mode | Stages | Notes |
|---|---|---|
| `tracked` (default) | git-tracked files, including uncommitted modifications | Untracked files are left out and listed as a warning — `git add` them or switch modes to include them |
| `committed` | the same files as `tracked` | Fails if the worktree has uncommitted changes or untracked files; use for reproducible, reviewable source packages |
| `worktree` | everything that isn't git-ignored, tracked or not | |

If the source directory is not a git worktree, `tracked` and `committed` fall back to `worktree` with a warning.
The [output dir](#output-dir) is never staged.
Git submodules are skipped with a note, since their contents aren't tracked by the parent repository.

To persist a mode, set `source_sync_mode = "committed"` in [`debmagic.toml`](config.md).

## Output dir

Build artifacts land in `build/` under the package root (`output_dir`, `-o`).
debmagic marks the output dirs it creates with a [`CACHEDIR.TAG`](https://bford.info/cachedir/), so backup tools skip them.
The tag also tells debmagic's `build/` apart from one an upstream project ships: an existing `build/` without the tag that holds more than build artifacts belongs to upstream.
debmagic then uses `build-debmagic/` instead (`output_dir_fallback`), or stops with `output_dir_clash = "fail"`.
`-o` is always used as given.

## Repeating a build

You can iterate on the same build for faster compile times.

### Persistent container

Every `debmagic build binary` keeps the Environment when the build command fails (`--persistent=on-failure`, the default) and tears it down when the build succeeds. Pass `--persistent` (that is, `--persistent=always`) to keep it after success too, and reuse it on the next build while restaging the source tree:

```shell
debmagic build binary --driver lxd --persistent \
  --source-dir . --output-dir /tmp/out
```

`--persistent=no` discards a kept Environment and starts fresh. A later `on-failure` success also tears down an Environment a previous `always` left behind.

### Incremental builds

Use `--incremental` to retain the environment and synchronize only source changes while preserving generated files and unchanged source inodes.
This flag forces `--persistent=always`, including over an explicit `no` or `on-failure`, and cannot be combined with `--clean yes`.

The preserved build tree is kept even when the environment itself is *not* reused (e.g. a fresh CI runner where the tree was restored from a cache).
Quilt patches that a previous build or shell session left applied in the build tree are unapplied with `dpkg-source --after-build` before sources are synced, so the worktree remains the source of truth and stale `.pc` state cannot break later builds.

The Environment's Host root lives under `environments_dir` (default `$XDG_DATA_HOME/debmagic/environments`). Build artifacts, including `.changes` files, are exported to `output_dir`.


## Selecting a distro/release

Only needed when `debian/changelog`'s top entry doesn't unambiguously determine the target: pass `--distro <codename>` (e.g. `--distro noble`, `--distro trixie`).
If the changelog has a single unambiguous entry, omit it.

`--distro` also overrides the changelog's distribution: you can use it to rebuild a package released for an older release on a newer one, or to attempt a backport.
On the Bare driver the target must still match the host's os-release; pass `--bare-ignore-release` to build for a different suite anyway, with the host providing the build dependencies itself.

Suite aliases in the changelog (or via `--distro`) resolve to a concrete release: Debian `stable` / `oldstable` / `testing` / `sid` (→ `unstable`), and Ubuntu `devel`.

The releases and their dates come from [distro-info-data](https://salsa.debian.org/debian/distro-info-data) in `/usr/share/distro-info`.
Without it, or when it predates the latest release, debmagic fetches the data into `~/.cache/debmagic/distro-info/` and refreshes that copy every 30 days.

Non-Debian/Ubuntu suites (still apt/dpkg-based) are supported when declared for the active container Driver via `base_images`, e.g. `driver.docker.base_images = { "yocto:kirkstone" = "my-registry/yocto-kirkstone:latest" }`. The changelog/`--distro` value stays the bare codename (`kirkstone`). On the Bare driver, binary builds require the host `/etc/os-release` to match: built-in Debian/Ubuntu need matching `ID` and codename, other suites a matching `VERSION_CODENAME`.

## Proposed dependencies

Pass `--proposed` to build against the suite the release's updates come from, like Launchpad builds for `-proposed` do: everything newer there is installed and upgraded.

| Target | Proposed suite |
|---|---|
| Ubuntu release | `<release>-proposed` |
| Debian testing (devel release) | `unstable` |
| Debian stable release | `<release>-proposed-updates` |
| Debian `unstable`, `experimental` | none |

## Experimental

A build for Debian `experimental` installs its build dependencies from unstable.
Only when unstable can't satisfy them, the install is retried preferring the packages from experimental.

## Building debug symbol packages

By default `debmagic build binary` passes `DEB_BUILD_OPTIONS=noautodbgsym` to `dpkg-buildpackage`, which suppresses debhelper's automatic `-dbgsym` package (the detached debug info package debhelper otherwise builds by default from compat 9 onward).
Pass `--debug-symbols` to build it for one invocation:

```shell
debmagic build binary --debug-symbols --output-dir /tmp/out
```

Or set `build_debug_symbols = true` in the [`debmagic.toml`](config.md).

## Running tests

By default the build runs the package's test suite (the `test` stage of `debian/rules.py`, or `dh_auto_test` via the dh preset).
Pass `--test=no` to skip it for one invocation, or set `run_test = false` in the [`debmagic.toml`](config.md):

```shell
debmagic build binary --test=no
```

Boolean flags accept `yes`/`no`, `true`/`false` and `on`/`off`; a bare flag means `yes`.

This exports `DEB_BUILD_OPTIONS=nocheck`, the standard dpkg mechanism: dpkg-buildpackage propagates it into the build, debmagic's `test` stage is skipped, and classic debhelper packages skip `dh_auto_test` as usual.
It also adds the `nocheck` build profile to `DEB_BUILD_PROFILES`, so test-only build dependencies (`<!nocheck>`) aren't installed.

## Signing

`--sign` GPG-signs the resulting `.changes`/`.dsc`/`.buildinfo` after building — mainly useful for [source builds destined for Launchpad](source.md#uploading-to-launchpad), but works for binary builds too.

Without `--sign`, source builds are signed and binary builds are not (if `sign.key`, or `sign.tool = "custom"` are configured).
`build.sign_source` and `build.sign_binary` change that per kind; `--sign` signs regardless, and `--sign=no` never does.

```toml
[build]
sign_binary = true   # also sign binary builds
sign_source = false  # but not source builds
```

Signing is debmagic's own reimplementation of `debsign` and always runs on the host with your gpg keyring: the artifacts are exported to the host output dir first, so no container or agent forwarding is involved.
Children are signed first (`.dsc`, then `.buildinfo`) and the `.changes` checksums are rewritten after each, exactly like `debsign`.

The `--sign` mode decides what happens to an existing signature:

| Mode | Unsigned file | Signed by our key | Signed by another key |
|---|---|---|---|
| `no` | left alone | left alone | left alone |
| `keep` | signed | kept | kept |
| `auto` | signed | skipped | re-signed |
| `force` | signed | re-signed | re-signed |

`auto` is the sensible default: re-signing with the same key is pointless, but a foreign signature is replaced.
`keep` accepts any existing signature — useful when a `.changes` was already signed by something else.
`force` re-signs even what our key already signed, e.g. to switch to a new signature over the same content.

| Option | Config | Description |
|---|---|---|
| `--sign <mode>` | `build.sign_source`, `build.sign_binary`, `sign.source` | Sign after building: `no`, `keep`, `auto` (default) or `force`; the config decides what happens without the flag |
| `--sign-key <key>` | `sign.key` | Key ID/fingerprint/email; defaults to the `Changed-By:`/`Maintainer:` address of the file being signed |
| `--sign-tool <tool>` | `sign.tool` | OpenPGP implementation: `gpg` (default), `sequoia` (sq), or `custom` |
| `--sign-command <cmd>` | `sign.sign_command` | Custom signing command for `--sign-tool custom` (see below) |
| `--notify` | `sign.notify` | Desktop notification + terminal bell just before signing, so a hardware-key touch prompt isn't missed after a long build (see [Notifications](#notifications)) |

A custom signing command runs without a shell and must write the clearsigned result to stdout.
The file to sign is passed via the `{file}` placeholder (or, if no placeholder is used, as the last argument).

| Placeholder | Expands to |
|---|---|
| `{file}` | Path of the file to sign |
| `{key}` | The resolved signing key |
| `{email}` | The bare address of the key |

Unknown placeholders are an error.

Defaults can be set in [`debmagic.toml`](config.md).

## Notifications

A desktop notification (`notify-send`) plus a terminal bell tells you when a long build needs you.
The top-level `notify` setting is the default for each step; `build.notify` and `sign.notify` override it per step:

```toml
notify = true

[sign]
notify = false  # no touch prompt notification, but "build finished"
```

A build that signs notifies once, when signing starts: "build finished, signing" if `build.notify` is on, else just the touch prompt if `sign.notify` is.
An unsigned build notifies when it finishes, a failed one in any case.
`--notify[=no]` overrides all steps of one run.

## Signing an existing build

`debmagic sign` signs a `.changes` file (and its `.dsc`/`.buildinfo` children) that already exists — the same code path `--sign` uses after a build:

```shell
debmagic sign ../mypkg_1.0_amd64.changes
```

Without a file argument, it locates the `.changes` via `debian/changelog` and `--output`/`-o` (default: the `output_dir` config, `build/` under the package root), preferring the source-only `_source.changes` when several match.

## Cleaning

`--clean` runs `debian/rules clean` before building, like plain `dpkg-buildpackage` does unless passed `-nc`; `--clean=no` skips it even if the config file defaults to cleaning.
Non-incremental builds already stage a clean source tree, while incremental builds preserve outputs intentionally.
Enable cleaning only for packages whose `clean` target performs required setup or code generation.

## Persisting options in a config file

Instead of repeating CLI flags on every invocation, drop a [config file](config.md).

## Internals

- Container/device names are derived and sanitized internally (alphanumeric + hyphen, ≤63 chars for LXD/Incus) — don't try to predict or construct them yourself; use `debmagic env shell` instead of `lxc`/`docker` commands directly.
