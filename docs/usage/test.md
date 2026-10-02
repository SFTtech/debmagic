# Running package tests

Quick reference for running a package's declared Debian autopkgtest tests with `debmagic test`.

## TL;DR

- Entry point: `debmagic test` — runs tests from `debian/tests/control`; needs a prior `debmagic build`
- Requires a completed binary build (looked up from the Environment registry's Invocation history, or pass `--changes`)

```shell
cd your-package
debmagic build binary --driver docker
debmagic test --driver docker
```

## What it does

`debmagic test` installs the binary packages from a prior build and runs the package's declared autopkgtest tests (`debian/tests/control`)
inside a **fresh, separate** driver-managed environment.
The test environment is never the build environment — even when Persistence `on-failure` or `always` reuses a container across runs,
the test tree is reset and the `.debs` are reinstalled each time.

The driver *is* the testbed: `autopkgtest` runs with the `null` backend inside the container (or on the host for the bare driver). No `autopkgtest-virt-*` backends are used.

## Available options

| Option | Description |
|---|---|
| `--driver <...>` | Test environment driver (defaults to the driver recorded on the prior binary-build Invocation) |
| `--persistent <mode>` | How long the test environment outlives this run: `on-failure` (default) keeps it when the tests fail, `always` keeps it after every run, `no` tears it down. A bare `--persistent` means `always` |
| `--strict` | Treat skipped tests and "no tests declared" as failures (exit code 2) |
| `--changes <path>` | Path to a `.changes` file whose directory supplies the built `.debs` (for pipeline use). Overrides the artifact path only; driver and distro still default to the prior binary-build Invocation |
| `--distro <name>` | Override the target distro for the test environment (defaults to the prior binary-build Invocation's distro, not the changelog) |
| `--proposed` | Enable the `<release>-proposed` pocket in the test environment |
| `--apt-mirror <url>` | Mirror URL (same as [`debmagic build`](build.md)) |
| `--apt-update-age <when>` | Configure when to run `apt-get update` (same as [`debmagic build`](build.md#apt-update-age)) |
| `--source-dir <dir>` | Directory containing the `debian/` package directory |
| `--allow-host-test` | Allow the bare driver, which runs autopkgtest as root on the host |

Driver-specific flags (`--driver-docker-base-image`, `--driver-lxd-*`) mirror `debmagic build`.

## Picking a driver

Use the same drivers as for builds. Pass `--driver` explicitly (or rely on the driver recorded on the prior binary-build Invocation):

| Driver | Isolation the Environment provides |
|---|---|
| `lxd` / `incus` | Container (`isolation-container`) |
| `docker` | Container (`isolation-container`) |
| `bare` | None — tests run as root on the host; requires `--allow-host-test` |

The driver *is* the testbed, so autopkgtest is told to run tests whose isolation restrictions the environment actually satisfies (`--ignore-restrictions`, only for those rungs). Tests that declare `Restrictions: isolation-container` therefore run on Docker/LXD/Incus instead of skipping. `isolation-machine` is not provided by any current driver (none is a VM); those tests still skip. Bare provides nothing, even with `--allow-host-test`.

## Exit codes

| Code | Meaning |
|---|---|
| `0` | All tests passed, or skips/no-tests were allowed |
| `1` | Test failure, testbed error, or other autopkgtest error |
| `2` | Strict-only failure: skipped tests or no tests declared under `--strict` |

autopkgtest skips tests whose `Restrictions:` the Environment cannot satisfy (today: `isolation-machine` on every current driver). Skips are reported loudly; use `--strict` to escalate them to exit code 2.

If no `debian/tests/control` exists (or it declares no tests), the run exits 0 with a notice — or exit 2 under `--strict`.

## Inspecting a failed test run

When the tests themselves fail and this run's Persistence is `on-failure` (the default) or `always`, the Environment stays and debmagic prints `debmagic env shell <id>`. A strict skip tears the Environment down. `no` always tears it down and prints no hint.

Test output and logs are exported to a `<changes name>.test/` directory next to the `.changes` file (for `pkg_1.0-1_amd64.changes`, that is `pkg_1.0-1_amd64.test/`); the path is printed at the end of the run. A later run replaces that directory only if debmagic wrote it; otherwise the run fails rather than deleting it.

```shell
debmagic env shell <id>
```

## Prior build required

By default `debmagic test` looks up the latest successful binary-build Invocation for this Source tree whose exported `.changes` still exists. If more than one DistroVersion matches, it errors and lists them — pass `--distro`. If none are found:

run `debmagic build binary` first

Use `--changes` to supply a `.changes` file from an exported output directory. That overrides the artifact path only. Driver and distro still come from the prior binary-build Invocation when one exists; pass `--driver` and `--distro` when it does not. Several DistroVersions still require `--distro`.

## Bare driver

The bare driver runs autopkgtest as root directly on the host.
This violates the no-leak principle for normal use — pass `--allow-host-test` to opt in explicitly.
