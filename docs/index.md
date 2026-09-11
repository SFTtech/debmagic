# debmagic

![debmagic image](../assets/debmagic-logo.svg){width=250px align=center}

Modern, robust & easy [Debian](https://debian.org)/[Ubuntu](https://ubuntu.com) packaging - while staying backwards compatible.

Debmagic unifies the the packaging experience as a single, streamlined tool. Additionally, `debmagic-pkg` allows you to define package build recipes in Python.
Working with Debian packages without `debmagic` means juggling with several independent tools like `sbuild`/`pbuilder` chroots, `dch`, `dpkg-buildpackage`, `debsign`, `autopkgtest` virt-backends, `dput`, `lintian`, and various custom glue scripts.
Debmagic unifies this into one CLI with useful defaults:

- **Isolated builds without the setup**: `debmagic build binary` builds a `debian/`-packaged source tree in a temporary container (using LXD, Incus, Docker)
- **Fast iteration**: `--persistent`/`--incremental` reuse the environment and sync only source changes; `--shell-on-failure` and `debmagic shell` drop you right where the build broke
- **Test & sign integrated**: `debmagic test` runs the package's autopkgtest tests in a fresh environment, `debmagic sign` GPG-signs `.changes`/`.dsc`/`.buildinfo` on the host
- **Python packaging API**: replace complicated `debian/rules` Makefiles with typed Python `debian/rules.py`, with optional `dh` compatibility

[![CI](https://github.com/SFTtech/debmagic/actions/workflows/pull_request.yaml/badge.svg)](https://github.com/SFTtech/debmagic/actions)

## Quickstart

```shell
cd your-package   # any source tree with a debian/ directory
uvx debmagic build binary --driver docker
```

> [!TIP]
> `debmagic --help` lists everything.


Then dive into the pages below — start with [Getting started](usage/getting-started.md).

```{toctree}
:hidden:
:caption: Usage

usage/getting-started.md
usage/build.md
usage/test.md
usage/source.md
usage/config.md
usage/packaging.md
usage/modules/index.md
```

```{toctree}
:hidden:
:caption: Development

develop/index.md
develop/releasing.md
```

```{toctree}
:hidden:
:caption: 📖 Reference

develop/_changelog.md
```
