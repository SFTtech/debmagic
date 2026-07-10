# Building packages

Quick reference to build a Debian/Ubuntu package with `debmagic`.
It covers `debmagic build binary` — the generic entry point that works on *any* `debian/`-packaged source tree, including ones that don't use `debmagic-pkg` to write `debian/rules`.
For building just a `.dsc`/tarball without compiling anything, see [`debmagic build source`](source.md) instead.

## TL;DR

```shell
debmagic build binary --driver lxd \
  --source-dir /path/to/parent/of/debian/dir \
  --output-dir /path/to/put/the/.deb/files \
  --apt-mirror http://<fast-mirror-host>/ubuntu
```

- `--source-dir` is the directory *containing* `debian/`, not `debian/` itself.
  Defaults to the current directory.
- `--output-dir` is where the resulting `.deb`/`.udeb`/`.ddeb`, `.buildinfo` and `.changes` files end up.
  It's created if missing.
  Defaults to the current directory.
- `--driver` is required.
  Use `lxd` or `incus` if available (isolated containers); fall back to `docker`, then `bare` (builds directly on the host with no isolation, so only use this if you already trust the host environment).
- Exit code is non-zero on failure; stderr/stdout carry the real `dpkg-buildpackage`/`apt-get` output, so grep that for the actual error instead of guessing from the exit code alone.

## Picking a driver

Check what's installed and use the first that applies, in this order:

| Driver | Check it's available | Isolation |
|---|---|---|
| `lxd` / `incus` | `lxc list` / `incus list` | Full container isolation |
| `docker` | `docker info` | Full container isolation |
| `bare` | none (no daemon) | None — build-deps install with `sudo apt-get` directly on the host; only use in a disposable/CI environment |

There's no auto-detection; pick one and pass it explicitly every time.

## Speeding up builds with a mirror

Fresh containers install their base tooling plus every `Build-Depends`, so slow mirrors directly translate into slow builds.
Pass `--apt-mirror` to use a faster mirror for build-dependency resolution after the base tooling is bootstrapped from the image's configured archives:

```shell
debmagic build binary --driver lxd --apt-mirror http://<mirror-host>/ubuntu ...
```

Notes:

- Works for the `lxd`, `incus` and `docker` drivers.
  It's a no-op for `bare`, which uses the host's own apt sources.
- The image, mirror, proposed-pocket setting and host user IDs form the build-environment identity.
  Changing any of them automatically replaces an incompatible persistent container.
- Handles both the classic `sources.list` format and the deb822 `*.sources` format (Ubuntu 24.04+).
- To avoid repeating the flag, set it once in `debian/debmagic.toml` (see below) instead.

## Iterating on a build (faster repeat runs)

Every `debmagic build binary` invocation creates a new container by default and tears it down afterwards.
For repeated attempts against the same package and distro, add `--persistent` to retain and reuse the running environment while restaging the source tree for each build:

```shell
debmagic build binary --driver lxd --persistent \
  --source-dir . --output-dir /tmp/out
```

Use `--incremental` to retain the environment and synchronize only source changes while preserving generated files and unchanged source inodes.
Incremental mode is binary-only, implies `--persistent`, and cannot be combined with `--clean yes`.

## Inspecting a failed build

By default a failed build tears down the container, so nothing is left to inspect.
If a build might fail and you need to inspect it afterwards, pass `--persistent` up front, then once the run finishes:

```shell
debmagic shell --source-dir /path/to/parent/of/debian/dir
```

This attaches an interactive shell inside the still-running (or restartable) build environment, at the package's build directory.

## Selecting a distro/release

Only needed when `debian/changelog`'s top entry doesn't unambiguously determine the target: pass `--distro <codename>` (e.g. `--distro noble`, `--distro trixie`).
If the changelog has a single unambiguous entry, omit it.

## Building debug symbol packages

By default `debmagic build binary` passes `DEB_BUILD_OPTIONS=noautodbgsym` to `dpkg-buildpackage`, which suppresses debhelper's automatic `-dbgsym` package (the detached debug info package debhelper otherwise builds by default from compat 9 onward).
Pass `--debug-symbols` to build it for one invocation:

```shell
debmagic build binary --driver lxd --debug-symbols --source-dir . --output-dir /tmp/out
```

Or set `build_debug_symbols = true` in `debian/debmagic.toml`/`$XDG_CONFIG_HOME/debmagic/config.toml` to always build it.

## Signing and cleaning

`--sign yes` (plus optionally `--sign-key you@example.com`) GPG-signs the resulting `.changes`/`.dsc`/`.buildinfo` with `debsign` after building — always on the host, using your own gpg keyring, regardless of `--driver`.
This is mainly useful for [source builds destined for Launchpad](source.md#uploading-to-launchpad), but works for binary builds too.

`--clean yes` runs `debian/rules clean` before building, like plain `dpkg-buildpackage` does unless passed `-nc`.
Non-incremental builds already stage a clean source tree, while incremental builds preserve outputs intentionally.
Enable cleaning only for packages whose `clean` target performs required setup or code generation.

Both default to the `sign_package`/`sign_key`/`clean` settings in the config file (see below) if not passed on the CLI.

## Persisting options in `debian/debmagic.toml`

Instead of repeating CLI flags on every invocation, drop a config file next to `debian/rules`:

```toml
build_debug_symbols = true
sign_package = true
sign_key = "you@example.com"
clean = false

[driver]
persistent = true
apt_mirror = "http://<mirror-host>/ubuntu"

[driver.lxd]
# project = "my-project"
```

Config precedence (highest wins): `--config <file>` on the CLI > `<source-dir>/debian/debmagic.toml` > `$XDG_CONFIG_HOME/debmagic/config.toml`.
CLI flags like `--apt-mirror`/`--persistent`/`--sign`/`--clean` always override the matching config file value for that one invocation.

## What NOT to expect yet

- `debmagic test` and `debmagic check` are not implemented yet — don't rely on them for lintian/test output.
  Rely on `debmagic build binary`'s own `dpkg-buildpackage` run (which already runs `dh_auto_test` unless the package's `debian/rules` disables it).
- Container/device names are derived and sanitized internally (alphanumeric + hyphen, ≤63 chars for LXD/Incus) — don't try to predict or construct them yourself; use `debmagic shell` instead of `lxc`/`docker` commands directly.
