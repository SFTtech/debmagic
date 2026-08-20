# Running package tests

Quick reference for running a package's declared Debian autopkgtest tests with `debmagic test`.

## TL;DR

- Entry point: `debmagic test` — runs tests from `debian/tests/control`; needs a prior `debmagic build`
- Requires a completed build in the same build root (or pass `--changes` to point at exported artifacts)

```shell
cd your-package
debmagic build binary --driver docker
debmagic test --driver docker
```

## What it does

`debmagic test` installs the binary packages from a prior build and runs the package's declared autopkgtest tests (`debian/tests/control`)
inside a **fresh, separate** driver-managed environment.
The test environment is never the build environment — even when `--persistent` reuses a container across runs,
the test tree is reset and the `.debs` are reinstalled each time.

The driver *is* the testbed: `autopkgtest` runs with the `null` backend inside the container (or on the host for the bare driver). No `autopkgtest-virt-*` backends are used.

## Available options

| Option | Description |
|---|---|
| `--driver <...>` | Test environment driver (defaults to the driver recorded in the prior build's `environment.json`) |
| `--persistent` | Retain the test environment after the run for reattach/debug |
| `--strict` | Treat skipped tests and "no tests declared" as failures (exit code 2) |
| `--changes <path>` | Path to a `.changes` file whose directory supplies the built `.debs` (for pipeline use) |
| `--distro <name>` | Override the target distro for the test environment (defaults to the prior build's distro from `environment.json`, not the changelog) |
| `--proposed` | Enable the `<release>-proposed` pocket in the test environment |
| `--apt-mirror <url>` | Mirror URL (same as [`debmagic build`](build.md)) |
| `--source-dir <dir>` | Directory containing the `debian/` package directory |
| `--allow-host-test` | Allow the bare driver, which runs autopkgtest as root on the host |
| `--shell-on-failure` | On test failure, drop into an interactive shell in the test environment when stdout is a TTY |

Driver-specific flags (`--driver-docker-base-image`, `--driver-lxd-*`) mirror `debmagic build`.

## Picking a driver

Use the same drivers as for builds. Pass `--driver` explicitly (or rely on the driver recorded in the prior build's `environment.json`):

| Driver | Isolation |
|---|---|
| `lxd` / `incus` | Full container isolation |
| `docker` | Full container isolation |
| `bare` | None — tests run as root on the host; requires `--allow-host-test` |

## Exit codes

| Code | Meaning |
|---|---|
| `0` | All tests passed, or skips/no-tests were allowed |
| `1` | Test failure, testbed error, or other autopkgtest error |
| `2` | Strict-only failure: skipped tests or no tests declared under `--strict` |

autopkgtest skips tests whose `Restrictions:` the `null` backend cannot satisfy (e.g. `isolation-container`, `isolation-machine`). Skips are reported loudly; use `--strict` to escalate them to exit code 2.

If no `debian/tests/control` exists (or it declares no tests), the run exits 0 with a notice — or exit 2 under `--strict`.

## Inspecting a failed test run

On failure the test environment is torn down by default. Pass `--shell-on-failure` to drop into an interactive shell inside the test environment when stdout is a TTY (destroyed on shell exit unless `--persistent` was used).

Test output and logs are exported to a `test/` subdirectory of the build root; the path is printed at the end of the run.

## Prior build required

By default `debmagic test` resolves the prior build from the build root (same layout as `debmagic shell`). If no build artifacts are found:
run `debmagic build` first

Use `--changes` to supply a `.changes` file from an exported output directory instead.

## Bare driver

The bare driver runs autopkgtest as root directly on the host.
This violates the no-leak principle for normal use — pass `--allow-host-test` to opt in explicitly.
