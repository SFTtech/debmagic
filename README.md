# Debmagic

<img align="right" style="float: right; width: 25%;" src="assets/debmagic-logo.svg" alt="debmagic logo"/>

Debmagic is for [Debian](https://debian.org)/[Ubuntu](https://ubuntu.com):

- create packages **build instructions** in Python with `debian/rules.py`
- tooling do perform packaging itself: **building** and **testing** in isolated container environments

[![GitHub Actions Status](https://github.com/SFTtech/debmagic/actions/workflows/ci.yml/badge.svg)](https://github.com/SFTtech/debmagic/actions/workflows/ci.yml)

> [!IMPORTANT]
> Debmagic's goal: make Debian packaging modern, robust & easy - while being backwards compatible.

---

Included features:

- for `debian/rules.py`
  - language and buildsystem [helper modules](src/debmagic/_module)
- maintainer tools
  - `debmagic build` - isolated package building

## Example debian/rules.py

Python `debian/rules.py` equivalent of [Ubuntu 24.04 htop](https://git.launchpad.net/ubuntu/+source/htop/tree/debian/rules?h=ubuntu/noble):

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

### debhelper compatibility

Debmagic can use `dh` and provides **dh overrides** as common in `debian/rules` Makefiles:

```python
from debmagic.v0 import dh

# specify dh arguments:
dhp = dh.Preset("--with=python3 --builddirectory=build")
pkg = package(preset=dhp)

# define optional overrides:
@dhp.override
def dh_auto_install(build: Build):
    print("dh override worked :)")
    build.cmd("dh_auto_install --max-parallel=1")

pkg.pack()
```

### Custom functions

To add custom functions directly usable from CLI (like custom `debian/rules` targets for maintainers):

```python
pkg = package(...)

@pkg.custom_function
def something_custom(some_param: int, another_param: str = "some default"):
    print(f"you passed {some_param=} {another_param=}")

pkg.pack()
```

This function can be directly called with:

```console
./debian/rules.py something-custom --another-param=test 1337
```

```text
you passed some_param=test another_param=1337
```

And generates automatic help for:

```console
./debian/rules.py something-custom --help
```

## Documentation

To do packaging with debmagic, please **[read the documentation!](https://debmagic.readthedocs.io)**.

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
