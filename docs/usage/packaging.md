# Packaging with debmagic

`debmagic-pkg` allows you to write a package build recipe in Python.


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

For even more straightforward conversion of `debian/rules` Makefiles, Debmagic can run [`dh` sequences](packages/debmagic-pkg/src/debmagic/v0/_module/dh.py) and provides **dh overrides**:

```python
from debmagic.v0 import dh

# specify dh arguments:
dhp = dh.Preset("--with=python3 --builddirectory=build")
pkg = package(preset=dhp)

# if needed, define optional overrides:
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
