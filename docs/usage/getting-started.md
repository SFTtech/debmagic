# Getting Started

## Installation

### Cargo

```shell
cargo install debmagic
```

### Pip

Run it directly with [`uv`](https://docs.astral.sh/uv/):

```shell
uvx debmagic
```

or install using `pip`:
```shell
pip install debmagic
```


### Debian / Ubuntu - Soon (tm)

```shell
apt install debmagic
```

## Building an existing package (CLI)

`debmagic build` builds *any* Debian-packaged source tree inside a throwaway build environment, driven by a build driver.

```shell
cd your-package   # any source tree with a debian/ directory
debmagic build binary --driver docker
```

The driver picks the isolation technology — `lxd`, `incus`, `docker` (full container isolation) or `bare` (no isolation, for disposable/CI environments).
There's no auto-detection; pass one explicitly or set it in a [`debmagic.toml`](config.md).

From here:

- [Building packages](build.md) — all `debmagic build` options: drivers, distro selection, incremental builds, signing, ...
- [Running package tests](test.md) — `debmagic test` against a prior build
- [Building source packages](source.md) — `debmagic build source` and uploading to Launchpad
- [Uploading](upload.md) — `debmagic upload`, upload targets and pre-upload checks
- [Upstream management](upstream.md) — `debmagic upstream list`/`switch` for version bumps & backports
- [Configuration](config.md) — persistent settings in `debmagic.toml`
- [Creating package recipes](packaging.md) — writing `debian/rules.py` equivalents with `debmagic-pkg`