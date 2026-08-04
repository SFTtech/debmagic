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

| Key | Type | Default | Description |
|---|---|---|---|
| `driver.persistent` | bool | `false` | Keep and reuse the build environment across runs instead of tearing it down. |
| `driver.apt_mirror` | string | — | Mirror used for build-dependency resolution. Not used by the `bare` driver. |
| `driver.proposed` | bool | `false` | Also enable the `<release>-proposed` pocket. Not used by the `bare` driver. |
| `driver.docker.base_images` | map | — | Base image per distro, keyed by `"<distro>:<codename>"` (e.g. `"debian:trixie"`). Falls back to `docker.io/<distro>:<codename>`. |
| `driver.lxd.project` | string | — | LXD/Incus project to use. `None` uses the default project. |
| `driver.lxd.base_images` | map | — | Base image per distro, keyed by `"<distro>:<codename>"`. Falls back to the driver's default remote image. |
| `temp_build_dir` | path | `/tmp/debmagic` | Where build trees are staged. |
| `incremental` | bool | `false` | Retain the environment and sync only source changes, preserving generated files. Binary-only; implies `persistent`; incompatible with `clean`. |
| `source_sync_mode` | enum | `tracked` | Which source files are staged (see below). |
| `build_debug_symbols` | bool | `false` | Build the automatic `-dbgsym` debug symbol package. |
| `sign_package` | bool | `false` | Sign the resulting `.changes`/`.dsc` with `debsign`. |
| `sign_with` | enum | `auto` | Where `debsign` runs (see below). |
| `sign_key` | string | — | GPG key ID/email for `debsign -k`. Required for container signing. |
| `clean` | bool | `false` | Run `debian/rules clean` before building. Disabled by default; incompatible with `incremental`. |

### `source_sync_mode`

| Value | Stages |
|---|---|
| `tracked` (default) | Git-tracked files, including uncommitted modifications. Untracked files are left out and reported as a warning. |
| `committed` | Same files as `tracked`, but fails if the worktree has uncommitted changes or untracked files. |
| `worktree` | Everything that isn't git-ignored, tracked or not. |

### `sign_with`

| Value | Behavior |
|---|---|
| `auto` (default) | Sign on the host if `debsign` is available there, otherwise in a container. |
| `host` | Always sign on the host with `debsign`. |
| `same` | Sign inside a minimal same-distro container, forwarding the host's gpg-agent socket. Requires `sign_key`. |

## Example


```toml
build_debug_symbols = true
sign_package = true
sign_with = "same"
sign_key = "you@example.com or gpg key id"
clean = false

[driver]
persistent = true
apt_mirror = "http://<mirror-host>/ubuntu"

[driver.lxd]
# project = "my=lxd-project-id"
```
