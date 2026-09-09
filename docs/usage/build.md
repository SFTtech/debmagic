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
| `--persistent` | Retain the build environment. This does not do incremental builds. |
| `--incremental` | Do incremental builds by syncing changed sources only; implies `persistent` |
| `--distro <name>` | [Select the target distro/release](#selecting-a-distrorelease) (e.g. `trixie`, `resolute`) |
| `--proposed` | Use [build dependencies from `proposed`](#proposed-dependencies) pocket |
| `--sign` | [GPG-sign the resulting `.changes`/`.dsc`/`.buildinfo`](#signing) |
| `--clean` | Run [`debian/rules clean` before building](#cleaning) |
| `--debug-symbols` | [Build the automatic `-dbgsym` debug symbol packages](#building-debug-symbol-packages) |
| `--apt-mirror <url>` | [Mirror URL](#mirror-selection) |
| `--source-dir <dir>` | Directory containing the `debian/` package directory |
| `--output-dir <dir>` | Directory to put the resulting build artifacts |
| `--shell-on-failure` | On build failure, drop into an interactive shell in the build environment when stdout is a TTY |

[`debmagic shell`](#inspecting-a-failed-build) — attach an interactive shell to a build environment

## Picking a driver

Check what's installed and use the first that applies, in this order:

| Driver | Check it's available | Isolation |
|---|---|---|
| `lxd` / `incus` | `lxc list` / `incus list` | Full container isolation |
| `docker` | `docker info` | Full container isolation |
| `bare` | none (no daemon) | None — build-deps install with `sudo apt-get` directly on the host; only use in a disposable/CI environment |

There's no auto-detection; pick one and pass it explicitly every time (or configure it in a [`debmagic.toml`](config.md) file).

## Inspecting a failed build

On failure the build environment is torn down by default. Pass `--shell-on-failure` to drop into an interactive shell inside the build environment when stdout is a TTY (destroyed on shell exit unless `--persistent` was used).

To inspect after the run finishes, pass `--persistent` up front, then:

```shell
# if you're in the package still
debmagic shell
# from the outside:
debmagic shell --source-dir /path/to/parent/of/debian/dir
```

This attaches an interactive shell inside the still-running (or restartable) build environment, at the package's build directory.


## Mirror selection

Fresh containers install their base tooling plus every `Build-Depends`, so slow mirrors directly translate into slow builds.
Pass `--apt-mirror` to use a faster mirror for build-dependency resolution after the base tooling is bootstrapped from the image's configured archives:

```shell
debmagic build binary --driver lxd --apt-mirror http://<mirror-host>/ubuntu ...
```

You can persistently set this flag in  `.config/debmagic/config.toml`.

## Source file staging

Before building, `debmagic` stages the source tree into the build environment.
`--source-sync <mode>` controls which files are staged, so you always know what ends up in the build and in a generated source package:

| Mode | Stages | Notes |
|---|---|---|
| `tracked` (default) | git-tracked files, including uncommitted modifications | Untracked files are left out and listed as a warning — `git add` them or switch modes to include them |
| `committed` | the same files as `tracked` | Fails if the worktree has uncommitted changes or untracked files; use for reproducible, reviewable source packages |
| `worktree` | everything that isn't git-ignored, tracked or not | |

If the source directory is not a git worktree, `tracked` and `committed` fall back to `worktree` with a warning.
Git submodules are skipped with a note, since their contents aren't tracked by the parent repository.

To persist a mode, set `source_sync_mode = "committed"` in [`debmagic.toml`](config.md).

## Repeating a build

You can iterate on the same build for faster compile times.

### Persistent container

Every `debmagic build binary` invocation creates a new container by default and tears it down afterwards.
For repeated attempts against the same package and distro, add `--persistent` to retain and reuse the running environment while restaging the source tree for each build:

```shell
debmagic build binary --driver lxd --persistent \
  --source-dir . --output-dir /tmp/out
```

### Incremental builds

Use `--incremental` to retain the environment and synchronize only source changes while preserving generated files and unchanged source inodes.
This flag implies `--persistent`, and cannot be combined with `--clean yes`.

The preserved build tree is kept even when the environment itself is *not* reused (e.g. a fresh CI runner where the tree was restored from a cache).


## Selecting a distro/release

Only needed when `debian/changelog`'s top entry doesn't unambiguously determine the target: pass `--distro <codename>` (e.g. `--distro noble`, `--distro trixie`).
If the changelog has a single unambiguous entry, omit it.

Suite aliases in the changelog (or via `--distro`) resolve to a concrete release: Debian `stable` / `oldstable` / `sid` (→ `unstable`), and Ubuntu `devel`.
Alias targets are updated manually when Debian/Ubuntu roll.

Non-Debian/Ubuntu suites (still apt/dpkg-based) are supported when declared for the active container Driver via `base_images`, e.g. `driver.docker.base_images = { "yocto:kirkstone" = "my-registry/yocto-kirkstone:latest" }`. The changelog/`--distro` value stays the bare codename (`kirkstone`). On the Bare driver, the host `/etc/os-release` must match: built-in Debian/Ubuntu need matching `ID` and codename; other suites need a matching `VERSION_CODENAME` only.

## Proposed dependencies

If needed, build dependencies can be used from `<release>-proposed`.
Pass `--proposed` to enable the proposed pocket in the build environment.

## Building debug symbol packages

By default `debmagic build binary` passes `DEB_BUILD_OPTIONS=noautodbgsym` to `dpkg-buildpackage`, which suppresses debhelper's automatic `-dbgsym` package (the detached debug info package debhelper otherwise builds by default from compat 9 onward).
Pass `--debug-symbols` to build it for one invocation:

```shell
debmagic build binary --debug-symbols --output-dir /tmp/out
```

Or set `build_debug_symbols = true` in the [`debmagic.toml`](config.md).

## Signing

`--sign` GPG-signs the resulting `.changes`/`.dsc`/`.buildinfo` after building — mainly useful for [source builds destined for Launchpad](source.md#uploading-to-launchpad), but works for binary builds too.
Signing is debmagic's own reimplementation of `debsign` and always runs on the host with your gpg keyring: the artifacts are exported to the host output dir first, so no container or agent forwarding is involved.
Children are signed first (`.dsc`, then `.buildinfo`) and the `.changes` checksums are rewritten after each, exactly like `debsign`.

| Option | Config | Description |
|---|---|---|
| `--sign` | `sign.source` | Sign after building; `--sign=false` skips it for one invocation |
| `--sign-key <key>` | `sign.key` | Key ID/fingerprint/email; defaults to the `Changed-By:`/`Maintainer:` address of the file being signed |
| `--sign-tool <tool>` | `sign.tool` | OpenPGP implementation: `gpg` (default), `sequoia` (sq), or `custom` |
| `--sign-command <cmd>` | `sign.command` | Custom signing command for `--sign-tool custom` (see below) |
| `--sign-notify` | `sign.notify` | Desktop notification + terminal bell just before signing, so a hardware-key touch prompt isn't missed after a long build |

A custom signing command runs without a shell and must write the clearsigned result to stdout.
The file to sign is passed via the `{file}` placeholder (or, if no placeholder is used, as the last argument).

| Placeholder | Expands to |
|---|---|
| `{file}` | Path of the file to sign |
| `{key}` | The resolved signing key |
| `{email}` | The bare address of the key |

Unknown placeholders are an error.

Defaults can be set in [`debmagic.toml`](config.md).

## Signing an existing build

`debmagic sign` signs a `.changes` file (and its `.dsc`/`.buildinfo` children) that already exists — the same code path `--sign` uses after a build:

```shell
debmagic sign ../mypkg_1.0_amd64.changes
```

Without a file argument, it locates the `.changes` via `debian/changelog` and `--output`/`-o` (default: the `output_dir` config, `build/` under the package root), preferring the source-only `_source.changes` when several match.

## Cleaning

`--clean` runs `debian/rules clean` before building, like plain `dpkg-buildpackage` does unless passed `-nc`; `--clean=false` skips it even if the config file defaults to cleaning.
Non-incremental builds already stage a clean source tree, while incremental builds preserve outputs intentionally.
Enable cleaning only for packages whose `clean` target performs required setup or code generation.

## Persisting options in a config file

Instead of repeating CLI flags on every invocation, drop a [config file](config.md).

## Internals

- Container/device names are derived and sanitized internally (alphanumeric + hyphen, ≤63 chars for LXD/Incus) — don't try to predict or construct them yourself; use `debmagic shell` instead of `lxc`/`docker` commands directly.
