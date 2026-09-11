# Debmagic

<img align="right" style="float: right; width: 25%;" src="assets/debmagic-logo.svg" alt="debmagic logo"/>

Modern, robust & easy [Debian](https://debian.org)/[Ubuntu](https://ubuntu.com) packaging - while staying backwards compatible.

Debmagic unifies the the packaging experience as a single, streamlined tool. Additionally, `debmagic-pkg` allows you to define package build recipes in Python.

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

### Commands

| Command | Goal |
| - | - |
| `debmagic build binary` | Build a binary package (`.deb`) in an isolated environment |
| `debmagic build source` | Create a source package (`.dsc`) for upload (incl signing) |
| `debmagic test` | Run the package's autopkgtest tests (`debian/tests/`) against a prior build |
| `debmagic shell` | Attach an interactive shell to the build environment |
| `debmagic sign` | GPG-sign a `.changes` file (and its `.dsc`/`.buildinfo`) on the host |
| `debmagic config` | Inspect and edit the effective `debmagic.toml` configuration |
| `debmagic check` | Lint the package *(in progress)* |


---

## Documentation

To learn using debmagic, follow **[the documentation!](https://debmagic.readthedocs.io)**.

## Debmagic package recipes

You can use the debmagic pkg API to create package build instructions (`debian/rules.py`), using [Debmagic API modules](packages/debmagic-pkg/src/debmagic/v0/_module/) for common build tools like `cargo`, `autotools`, `cmake`, `meson`, `go`, `ninja`, `python` `setup.py`/`pyproject.toml` and more.

While simple shell oneliners in a state-of-the art `debian/rules` **Makefile** can suffice for simple packages, packaging more complex projects like [openldap](https://git.launchpad.net/ubuntu/+source/openldap/tree/debian/rules?h=ubuntu/resolute-devel), [dovecot](https://git.launchpad.net/ubuntu/+source/dovecot/tree/debian/rules?h=ubuntu/resolute-devel), [samba](https://git.launchpad.net/ubuntu/+source/samba/tree/debian/rules?h=ubuntu/resolute-devel) or [gcc](https://git.launchpad.net/ubuntu/+source/gcc-15/tree/debian/rules?h=ubuntu/resolute-devel) can benefit from a more structured approach with `debmagic`.

To simplify the conversion of existing packages, we provide an optional `dh` sequence backward compatibility [module](packages/debmagic-pkg/src/debmagic/v0/_module/dh.py).

### Example debian/rules.py

For example, the [htop rules file from Ubuntu 24.04](https://git.launchpad.net/ubuntu/+source/htop/tree/debian/rules?h=ubuntu/noble) equivalent in debmagic native Python code could look like this:

```python
#!/usr/bin/env python3

from debmagic.v0 import Build, autotools, dh, package

pkg = package(
    preset=[dh],
    maint_options="hardening=+all",
)

if pkg.buildflags.DEB_HOST_ARCH_OS == "linux":
    configure_params = ["--enable-affinity", "--enable-delayacct"]
else:
    configure_params = ["--enable-hwloc"]

# hurd-i386 can open /proc (nothing there) and /proc/ which works
if pkg.buildflags.DEB_HOST_ARCH_OS == "hurd":
    configure_params += ["--with-proc=/proc/"]
else:
    configure_params += ["--enable-sensors"]


@pkg.stage
def configure(build: Build):
    autotools.configure(
        build,
        ["--enable-openvz", "--enable-vserver", "--enable-unicode", *configure_params],
    )

pkg.pack()
```


## Contributing

Debmagic can always use more features and modules!
You can also just request features or report bugs - this project is happy about your contributions!

- [Contributor documentation](https://debmagic.readthedocs.io/en/latest/develop/index.html)
- [Issue tracker](https://github.com/SFTtech/debmagic/issues)
- [Code contributions](https://github.com/SFTtech/debmagic/pulls)
- [Development roadmap](https://github.com/SFTtech/debmagic/projects)

## Contact

To directly reach developers and other users, we have chat rooms.
For questions, suggestions, problem support, please join and just ask!

| Contact       | Where?                                                                                         |
| ------------- | ---------------------------------------------------------------------------------------------- |
| Issue Tracker | [SFTtech/debmagic](https://github.com/SFTtech/debmagic/issues)                                 |
| Matrix Chat   | [`#sfttech:matrix.org`](https://app.element.io/#/room/#sfttech:matrix.org)                     |
| Support us    | [![donations](https://liberapay.com/assets/widgets/donate.svg)](https://liberapay.com/SFTtech) |

## License

Released under the **GNU General Public License** version 2 or later, [LICENSE](legal/GPL-2) for details.
