# Debmagic

Modern, robust & easy tooling for building and packaging [Debian](https://debian.org)/[Ubuntu](https://ubuntu.com) packages — while staying backwards compatible.

- **Build any package** in an isolated container environment with `debmagic build`
- **Run Debian autopkgtest tests** against built packages with `debmagic test`
- **Lint** with `debmagic lint`
- **Debug** build environments interactively with `debmagic shell`

## Installation

```shell
pip install debmagic
```

or run it directly:

```shell
uvx debmagic
```

## Quickstart

Build any Debian-packaged source tree in an isolated environment:

```shell
cd your-package
debmagic build binary --driver lxd \
  --output-dir /tmp/build-artifacts
```

Pick a driver explicitly — there's no auto-detection:

| Driver | Check it's available | Isolation |
|---|---|---|
| `lxd` / `incus` | `lxc list` / `incus list` | Full container isolation |
| `docker` | `docker info` | Full container isolation |
| `bare` | none (no daemon) | None — only use in a disposable/CI environment |

Create a source package (`.dsc`) without compilation:

```shell
debmagic build source
```

Run the package's declared Debian autopkgtest tests against a prior build:

```shell
debmagic build binary --driver docker
debmagic test --driver docker
```

Use `--strict` to fail on skipped or undeclared tests (exit code 2). The bare driver requires `--allow-host-test`.

### Useful options

- `--distro <codename>` — select the target distro/release (e.g. `trixie`, `noble`) if the changelog is ambiguous
- `--persistent` — retain the build environment for repeated attempts
- `--incremental` — sync only changed sources for faster rebuilds; implies `--persistent`
- `--sign` — GPG-sign the resulting `.changes`/`.dsc`/`.buildinfo` with `debsign`
- `--apt-mirror <url>` — use a faster mirror for build-dependency resolution

Any of these can be persisted in a `debmagic.toml` config file instead of repeating CLI flags.

### Inspecting a failed build

Failed builds tear down their environment by default. Pass `--shell-on-failure` to drop into a shell when stdout is a TTY, or build with `--persistent` and attach afterwards:

```shell
debmagic shell
```

## Documentation

For the full documentation — the [build quick reference](https://debmagic.readthedocs.io/en/latest/usage/build.html), 
packaging guides, configuration and module references — visit **[debmagic.readthedocs.io](https://debmagic.readthedocs.io)**.

## Acknowledgements

debmagic's lint implementation is heavily inspired by [oxlint](https://oxc.rs/docs/guide/usage/linter) and [Ruff](https://docs.astral.sh/ruff/). It draws on [lintian](https://lintian.debian.org/)’s checks and tag catalog, with the long-term goal of full lintian compatibility plus additional original rules.

## License

Released under the **GNU General Public License** version 2 or later.
