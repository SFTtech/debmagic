# Debmagic

Modern, robust & easy tooling for building and packaging [Debian](https://debian.org)/[Ubuntu](https://ubuntu.com) packages — while staying backwards compatible.

- **Build any package** in an isolated container environment with `debmagic build`
- **Run Debian autopkgtest tests** against built packages with `debmagic test`
- **Lint** with `debmagic check`
- **Debug** environments interactively with `debmagic env shell`

## Installation

```shell
cargo install debmagic
```

```shell
pip install debmagic
```

or run it directly with uv:

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
- `--persistent` — keep the build environment after every build (`always`); the default `on-failure` keeps it only when the build command fails
- `--incremental` — sync only changed sources for faster rebuilds; forces `--persistent=always`
- `--sign` — GPG-sign the resulting `.changes`/`.dsc`/`.buildinfo`
- `--apt-mirror <url>` — use a faster mirror for build-dependency resolution

Any of these can be persisted in a `debmagic.toml` config file instead of repeating CLI flags.

### Inspecting a failed build

A failed build or test keeps the Environment when Persistence is `on-failure` (the default) or `always`, and prints how to attach:

```shell
debmagic env shell <id>
```

## Documentation

For the full documentation — the [build quick reference](https://debmagic.readthedocs.io/en/latest/usage/build.html), 
packaging guides, configuration and module references — visit **[debmagic.readthedocs.io](https://debmagic.readthedocs.io)**.

## License

Released under the **GNU General Public License** version 2 or later.
