# Environments

The Environment registry is a machine-local index of every Environment created on this host (SQLite under `$XDG_DATA_HOME/debmagic/db.sqlite`). Host roots live under `environments_dir` (default `$XDG_DATA_HOME/debmagic/environments`). The Driver remains the source of truth for whether a registered Environment still exists. The file is schema-versioned: this debmagic migrates older registries in place, and refuses a file written by a newer one. `debmagic env list`, `debmagic env clean`, and `debmagic env shell` do not create the registry; with no file yet they report that nothing is registered. Keep the data directory on a local filesystem: the registry uses SQLite WAL and process ids, neither of which works across machines sharing a network home directory (set `DEBMAGIC_DATA_DIR` to a local path in that case).

```shell
debmagic env list
debmagic env shell
debmagic env shell <id>
debmagic env clean
debmagic env clean <id>
```

`debmagic environment` is the same command; `env` is the short alias. `debmagic shell` is removed.

## list

Prints every Environment in the registry after consolidating with the Driver:

| Column | Meaning |
|---|---|
| ID | Environment id (stable handle for `shell` / `clean`) |
| PURPOSE | `build` or `test` |
| DRIVER | Isolation backend |
| PACKAGE | Package name-version |
| DISTRO | Codename |
| PERSIST | Persistence of the claim that wrote the row: `no`, `on-failure`, or `always` |
| STATUS | `live`, `stale`, or `unreachable` |
| ATTACH | Live `debmagic env shell` Attachments |
| SOURCE | Source tree path |

**Stale** is derived on read: missing Driver resource, missing Host root, or a Persistence `no` Environment whose creating command is gone and that has no live Attachment. Persistence `on-failure` and `always` are not Stale merely because the command has exited. **Unreachable** means the Driver could not be queried (not Stale). A missing Source tree does not make an Environment Stale.

## shell

With no id, from a Source tree: attach if exactly one Environment exists for that checkout. Environments of the version currently in `debian/changelog` take precedence over those left from earlier versions. Zero → error. Two or more (including build + test) → error and print those Environments. From outside a Source tree, an Environment id is required.

An Attachment keeps a Persistence `no` Environment alive until the shell exits. While an Attachment is live, a build or test that would use the same Environment refuses to start instead of resetting it under the shell.

## clean

No arguments: destroy **Stale** Environments on the whole machine. Unreachable rows are skipped and reported. Each Environment is re-checked after it is claimed, so one that a concurrent run has just recreated is skipped rather than destroyed.

Each package version gets its own Environment, so Persistence `always` or a kept `on-failure` Environment stays until a later claim tears it down or it is cleaned by id; plain `env clean` does not remove healthy ones.

`debmagic env clean <id>` destroys that Environment even if it is healthy (Driver resource, Host root, registry row). It refuses while Attachments or a creating command are live, and refuses if the Environment is Unreachable — pass `--force` to destroy an Unreachable Environment anyway (its Driver resource may leak if the Driver cannot be queried).

If the Driver fails to destroy its resource (for example `docker rm` fails), the Environment stays in the registry with its Host root and is reported as an error (plain `env clean` reports it as skipped), so the resource is not lost; retry once the Driver works. With `--force`, the failure is only a warning and the Environment is removed from the registry anyway. A run tearing down its own Persistence `no` Environment, or an `on-failure` Environment whose command succeeded, behaves the same way as `env clean` without `--force`.
