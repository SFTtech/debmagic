# Building source packages

`debmagic build source` runs `dpkg-buildpackage -S -d -nc` to build a `.dsc`, tarball(s), `.buildinfo` and `.changes` without building binaries.
It's a target of the same [`debmagic build`](build.md) command that builds binary packages (`debmagic build binary`).

## TL;DR

```shell
# be in debian package directory, output to current directory:
debmagic build source

# or outside the debian package dir:
debmagic build source --source-dir /path/to/parent/of/debian/dir --output-dir /path/to/put/the/artifacts
```

- Same `--source-dir`/`--output-dir`/`--distro` semantics as [`debmagic build`](build.md).
- `--driver` defaults to `bare`: building a source package needs neither package build-dependencies nor a compiler, but the host must provide `dpkg-buildpackage` from `dpkg-dev`.
  Pass `--driver lxd`/`--driver incus`/`--driver docker` when the host does not provide a usable Debian build environment.
  Container drivers install their base tooling, but package build-dependencies are installed only with `--clean`.
  Binary builds (`debmagic build binary`) still require `--driver` to be passed explicitly.

(uploading-to-launchpad)=
## Uploading to Launchpad

```shell
# build, sign, then upload in one go:
debmagic build source --sign --sign-key you@example.com \
  --upload ppa:your-lp-username/your-ppa

# or upload an already-built .changes separately:
debmagic upload ppa:your-lp-username/your-ppa ./build/pkg_*_source.changes
```

- `--sign` GPG-signs the `.dsc`/`.buildinfo`/`.changes` after building, on the host with your gpg keyring — see [Signing](build.md#signing).
- `--sign-key` picks which key/uid to sign with; omit it to fall back to the `Changed-By:`/`Maintainer:` address of the file being signed.
- `--upload` picks the upload target (`name` or `name:parameter`), resolved from `[upload.targets]` over the builtins (`ppa`, `ubuntu`, `debian`) — see [Uploading](upload.md). Available on binary builds too.
- `--include-orig=auto|yes|no` controls whether the built `.changes` references the `orig` tarball (`-sa`/`-sd`): `auto` (default) includes it only when the archive cannot have it yet — a new upstream version or a deltarebase onto Debian. See [Upstream versions](upstream.md#including-the-orig-in-uploads).
- `--changes-option` adds an extra field to the `.changes` file, passed to `dpkg-buildpackage` as-is (repeatable). This is what git-ubuntu's `prepare-upload` uses to correlate source uploads with git history:

  ```shell
  debmagic build source --sign --sign-key you@example.com \
    --changes-option=-DVcs-Git=https://git.launchpad.net/~you/ubuntu/+source/pkg \
    --changes-option=-DVcs-Git-Ref=refs/heads/lp12345-fix \
    --changes-option=-DVcs-Git-Commit=36349e3669439603206cffc01dcb903b0ba2c134 \
    --upload ppa:you/your-ppa
  ```

- Both signing settings and upload targets can be set as defaults in `debian/debmagic.toml`/`$XDG_CONFIG_HOME/debmagic/config.toml` instead of passing them every time:

  ```toml
  [sign]
  source = true
  key = "you@example.com"
  ```

## What ends up in the source package

The same file selection as `debmagic build` uses to populate the build environment: everything under `--source-dir` except files matched by `.gitignore` (build artifacts, virtualenvs, ...).
Untracked-but-not-ignored files are included, so uncommitted work-in-progress changes are packaged too — useful while iterating locally.
`debian/source/options` (`tar-ignore`/`diff-ignore` patterns, etc.) is honored as usual, since it's `dpkg-source` itself that reads it.

Debian packages also require their `orig` tarball (the "upstream" sources).
For `3.0 (quilt)` packages the `orig` tarball is located automatically — see [Upstream versions](upstream.md#orig-tarballs-in-builds).

## What's NOT run

By default, `debian/rules` is never invoked (neither `dpkg-source` nor `dpkg-genchanges` need it), so this also works for source trees whose build-dependencies aren't installed anywhere.
Pass `--clean` to opt into running `debian/rules clean` first (like plain `dpkg-buildpackage` does unless passed `-nc`) — useful if a package's `clean` target is (ab)used for setup/codegen that should end up in the source package.
This also installs build-dependencies first, since the `clean` target usually needs its own tooling; a source build without `--clean` (the default) needs none of that.

## Known limitation: `--driver bare` on a non-Debian host

`dpkg-genbuildinfo` reads the dpkg status database (`/var/lib/dpkg/status`) to record installed package versions, unlike plain `dpkg-source`.
On a host that has `dpkg-dev` installed but isn't itself Debian/Ubuntu-based (or otherwise lacks a real dpkg database), this step fails with an error like `cannot open /var/lib/dpkg/status`.
Use `--driver lxd`/`--driver incus` in that case to build inside a proper Debian-ish container instead.

