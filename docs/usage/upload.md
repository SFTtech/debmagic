# Uploading

`debmagic upload` uploads a signed `.changes` file — and every file it references (`.dsc`, tarballs, `.buildinfo`) — to an archive or PPA, like `dput`.
It always uses ssh-based transports (`scp`/`sftp`), reusing your `~/.ssh/config` (host keys, agents, proxies, users) like any other ssh tool.

## TL;DR

```shell
# upload to your launchpad ppa (needs a signed .changes in the output dir):
debmagic upload ppa:your-lp-username/your-ppa

# or build, sign and upload in one go:
debmagic build source --sign --sign-key you@example.com --upload ppa:your-lp-username/your-ppa

# upload a specific .changes file to a host defined in debmagic.toml:
debmagic upload myhost ./build/pkg_1.0-1_source.changes
```

## Upload targets

The first argument is the *target*: `name` or `name:parameter` (split on the first `:`).

Targets resolve in this order:

1. `[upload.targets.<name>]` in `debmagic.toml`, merged field-by-field over a same-named builtin (so a config can override just e.g. `incoming` and keep the rest)
2. built-in targets:

| name | server | incoming | login | method |
|---|---|---|---|---|
| `ppa` | `ppa.launchpad.net` | `~{target}/ubuntu` | `anonymous` | sftp |
| `ubuntu` | `upload.ubuntu.com` | `ubuntu` | `anonymous` | sftp |
| `debian` | `ssh.upload.debian.org` | `/srv/upload.debian.org/UploadQueue` | ssh config | sftp |

The `:parameter` fills the `{target}` placeholder of the target's `incoming`/`server` — for `ppa:your-lp-username/your-ppa` the incoming dir becomes `~your-lp-username/your-ppa/ubuntu`.
Both PPAs and the Ubuntu archive accept sftp uploads.

A target with no builtin of that name is an error unless it's fully configured in `debmgic.toml`:

```toml
[upload.targets.myhost]
method = "sftp"        # or "scp"; unset keeps a builtin's method
server = "example.com"
incoming = "/srv/incoming"
login = "sfttech"     # optional; unset lets the ssh config decide
port = 2222            # optional; unset lets the ssh config decide
pre_upload_commands = [
  "lintian {changes}",
]
```

## Pre-upload checks

`pre_upload_commands` run *before anything is transferred*, each via `sh -c`:

- the `{changes}` placeholder is substituted with the `.changes` file path
- `DEBMAGIC_UPLOAD_CHANGES`, `DEBMAGIC_UPLOAD_TARGET`, `DEBMAGIC_UPLOAD_TARGET_SERVER` and `DEBMAGIC_UPLOAD_TARGET_INCOMING` are set in the environment
- a non-zero exit aborts the upload entirely

This is deliberately simple: it's a list of commands, not a plugin system.
Existing `dput-ng` hooks (which use its python "api") can be bridged by wrapping them in one command later, and debmagic's own built-in linter will simply be called from here too.

Pass `--no-hooks` to skip the checks.

## Including the `orig` tarball

`--include-orig=auto|yes|no` decides whether the upload carries the `orig` tarball (the `-sa`/`-sd` choice, made at upload time):

- `auto` (default): include only when the archive provably lacks this upstream version's orig (upstream version bump, deltarebase)
- `yes`/`no`: always/never

When the decision disagrees with the `.changes` file, its file listing is rewritten (orig entry added or removed, checksums recomputed) and it is re-signed.
Before adding, the tarball is verified against the checksum the `.dsc` recorded — a stale orig fails loudly instead of being rejected by the archive.
See [Upstream versions](upstream.md) for the full orig lifecycle.

## Upload log and `--force`

Every successful upload is recorded in a structured JSON file next to the `.changes`, e.g. `pkg_1.0-1_source.upload.json`:

```json
{
  "uploads": [
    { "target": "ppa:your-lp-username/your-ppa", "time": "2026-09-18T15:16:02+02:00" }
  ]
}
```

Uploading the same `.changes` to a target that already has a successful upload recorded is refused:

```shell
debmagic upload ppa:your-lp-username/your-ppa  # "already has a successful upload ..."
debmagic upload --force ppa:your-lp-username/your-ppa  # uploads anyway
```

The check is per target spec (including the `:parameter`), so uploading the same version to several PPAs works without `--force`.

## Upload methods

| method | how |
|---|---|
| `scp` | `scp -p [-P port] <file> <login@server>:<incoming>/<name>` |
| `sftp` | `sftp -b -` batch mode, `put`ting each file into `incoming` |

Both run over ssh and read `~/.ssh/config` themselves (user, port, keys, proxies), so only explicit `login`/`port` target settings are passed on the command line.

sftp paths are relative to the login's home on the server (`ubuntu` for the Ubuntu archive, `~your-lp-username/your-ppa/ubuntu` for PPAs), which is why the builtin `incoming` values above look like they do.

The methods share the [`Uploader`](https://debmagic.readthedocs.io) trait; a new method (e.g. git-based uploads) is a new config value plus one implementation.
