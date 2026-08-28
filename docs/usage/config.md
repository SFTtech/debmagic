# Configuration (`debmagic.toml`)

Persistent build settings live in a `debmagic.toml` file.
Every option has a sensible default, so the file is optional — add only what you want to change.

## Search order

Config files are merged in order of increasing precedence, so later files override earlier ones:

1. `~/.config/debmagic/config.toml` — your machine-wide defaults (e.g. a fast `apt_mirror`).
2. `<source_dir>/debian/debmagic.toml` — per-package settings, committed with the source.
3. An explicit `--config <file>` passed on the command line.

Only files that exist are read; missing ones are skipped.
Command-line flags override whatever the merged config resolves to.

## Options

All keys are optional.

| Key | Type | Default | CLI flag | Description |
|---|---|---|---|---|
| `driver.default` | enum | — | `--driver` | Build driver (`docker`, `bare`, `lxd`, `incus`) |
| `driver.persistent` | bool | `false` | `--persistent` | Keep and reuse the build environment across runs instead of tearing it down. |
| `driver.apt_mirror` | string | — | `--apt-mirror` | Mirror used for build-dependency resolution. Not used by the `bare` driver. |
| `driver.proposed` | bool | `false` | `--proposed` | Also enable the `<release>-proposed` pocket. Not used by the `bare` driver. |
| `driver.docker.base_images` | map | — | — | Base image per distro, keyed by `"<distro>:<codename>"` (e.g. `"debian:trixie"`). Falls back to `docker.io/<distro>:<codename>`. For non-Debian/Ubuntu suites (e.g. `"yocto:kirkstone"`), the map entry is what makes the suite a known DistroVersion for Docker builds. |
| `driver.lxd.project` | string | — | — | LXD/Incus project to use. |
| `driver.lxd.base_images` | map | — | — | Base image per distro, keyed by `"<distro>:<codename>"`. Falls back to the driver's default remote image. Same custom-suite registry role as Docker's map for LXD/Incus. |
| `temp_build_dir` | path | `/tmp/debmagic` | — | Where build trees are staged. |
| `incremental` | bool | `false` | `--incremental` | Retain the environment and sync only source changes, preserving generated files. Binary-only; implies `persistent`; incompatible with `clean`. |
| `source_sync_mode` | enum | `tracked` | `--source-sync` | Which source files are staged (see below). |
| `build_debug_symbols` | bool | `false` | `--debug-symbols` | Build the automatic `-dbgsym` debug symbol package. |
| `sign.source` | bool | `false` | `--sign`/`--no-sign` | Sign the resulting `.changes`/`.dsc` with `debsign` (see below). |
| `sign.with` | enum | `auto` | `--sign-with` | Where `debsign` runs (see below). |
| `sign.key` | string | — | `--sign-key` | GPG key ID/email for `debsign -k`. Required for container signing. |
| `sign.notify` | bool | `false` | `--sign-notify`/`--no-sign-notify` | Send a desktop notification via `notify-send` just before `debsign` runs, so a hardware-key touch prompt isn't missed. |
| `clean` | bool | `false` | `--clean`/`--no-clean` | Run `debian/rules clean` before building. Disabled by default; incompatible with `incremental`. |
| `shell_on_failure` | bool | `false` | `--shell-on-failure` | On build or test failure, drop into an interactive shell in the environment when stdout is a TTY. |
| `host_arch_variant` | string | — | `--host-arch-variant` | Build for a dpkg architecture variant (e.g. `"amd64v3"` on Ubuntu) -> `DEB_HOST_ARCH_VARIANT`. |

### `source_sync_mode`

| Value | Stages |
|---|---|
| `tracked` (default) | Git-tracked files, including uncommitted modifications. Untracked files are left out and reported as a warning. |
| `committed` | Same files as `tracked`, but fails if the worktree has uncommitted changes or untracked files. |
| `worktree` | Everything that isn't git-ignored, tracked or not. |

### `sign`

| Key | Type | Default | CLI flag | Description |
|---|---|---|---|---|
| `source` | bool | `false` | `--sign`/`--no-sign` | Sign the source package (`.changes`/`.dsc`) with `debsign`. |
| `with` | enum | `auto` | `--sign-with` | Where `debsign` runs (see below). |
| `key` | string | — | `--sign-key` | GPG key ID/email for `debsign -k`. Required for container signing. |
| `notify` | bool | `false` | `--sign-notify`/`--no-sign-notify` | Desktop notification via `notify-send` before signing. |

#### `sign.with`

| Value | Behavior |
|---|---|
| `auto` (default) | Sign on the host if `debsign` is available there, otherwise in a separate container. |
| `host` | Always sign on the host with `debsign`. |
| `build` | Sign inside the build container itself (no separate container is started). Requires a container driver and `sign.key`. |
| `separate` | Sign inside a minimal, separate same-distro container, forwarding the host's gpg-agent socket. Requires `sign.key`. |

## Example


```toml
build_debug_symbols = true
clean = false

[sign]
source = true
with = "separate"
key = "you@example.com or gpg key id"
notify = true

[driver]
default = "lxd"
persistent = true
apt_mirror = "http://<mirror-host>/ubuntu"

[driver.docker]
# Optional image overrides for known Debian/Ubuntu releases, and the registry
# for custom apt/dpkg suites (family:codename):
# base_images = { "debian:trixie" = "my-trixie:latest", "yocto:kirkstone" = "my-yocto:latest" }

[driver.lxd]
# project = "my=lxd-project-id"
```
