# Environments

Every build and test runs in an Environment: a container, VM or chroot that a Driver creates for one package version and distro.
debmagic remembers all Environments on this machine, so you can inspect, reuse and clean them up later.

```shell
debmagic env list               # what exists, and in what state
debmagic env shell [<id>]             # open the selected user's login shell
debmagic env exec [<id>] -- <command>...  # run a command directly
debmagic env clean              # destroy everything Stale
debmagic env rm <id>...         # destroy specific Environments
```

`env` is short for `debmagic environment`.

## list

Prints every Environment, after asking each Driver whether it still exists:

| Column | Meaning |
|---|---|
| ID | Environment id, used by `shell` and `rm` |
| PURPOSE | `build` or `test` |
| DRIVER | Isolation backend |
| PACKAGE | Package name and version |
| DISTRO | Codename |
| PERSIST | Persistence of the run that created it: `no`, `on-failure`, or `always` |
| STATUS | `live`, `stale`, or `unreachable` |
| ATTACH | Number of open `debmagic env shell` sessions |
| SOURCE | Source tree path |

An Environment is **Stale** when its Driver resource or Host root is gone, or when it has Persistence `no` and neither its creating command nor a shell is still running.
`on-failure` and `always` Environments stay `live` after their command exits.
**Unreachable** means the Driver could not be queried, so debmagic cannot tell whether the Environment still exists.

## shell

```shell
debmagic env shell        # inside a Source tree: its only Environment
debmagic env shell <id>   # anywhere
debmagic env shell -u 1000
```

The interactive command is the selected user's login shell from the Environment's passwd database.
Runs as `root` in containers by default, or as the invoking user with the bare driver; override with `--user`.

## exec

```shell
debmagic env exec [<id>] -- dpkg -l 'libfoo*'
debmagic env exec -u 1000 -- sh -c 'echo $HOME'  # as the build user
```

The command runs directly, without an implicit shell, and debmagic returns its exit code.
It gets a terminal only when debmagic has one, so pipes and redirections work.
Use `sh -c` explicitly when shell expansion is wanted.
Environment selection and `--user` work as for `shell`.

Without an id, debmagic picks the Environment of the current Source tree.
Environments of the version in `debian/changelog` take precedence over those of earlier versions.
If there are none or several (for example a build and a test Environment), it lists them and asks for an id.

Each call opens a new shell, so you can open several.
While a shell is open, the Environment is kept, even with Persistence `no`, and a build or test that would reuse it refuses to start instead of resetting it under you.

## clean and rm

Each package version gets its own Environment, so kept ones accumulate.

`debmagic env clean` destroys all **Stale** Environments on the machine and leaves healthy ones alone.
Unreachable Environments are skipped and reported.
An Environment that a concurrent run has just recreated is skipped too.

`debmagic env rm <id>...` (alias `remove`) destroys the given Environments, even healthy ones.
It refuses while a shell or the creating command is still running, and refuses Unreachable Environments.
The remaining ids are still processed when one fails.

If the Driver fails to destroy its resource (for example `docker rm` fails), the Environment stays listed so the resource is not lost; retry once the Driver works.
`--force` removes the Environment anyway, also when it is Unreachable, and only warns if the Driver resource may have leaked.

## Data directory

The registry and the Host roots live under `$XDG_DATA_HOME/debmagic` (see [`environments_dir`](config.md)).
Keep this on a local filesystem.
With a network-mounted home directory, set `DEBMAGIC_DATA_DIR` to a local path.
