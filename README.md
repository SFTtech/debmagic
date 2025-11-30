# Debmagic

Debian build instructions written in Python.

> Explicit is better than implicit.

```python
#!/usr/bin/env python3

from debmagic.v0 import Build, package
from debmagic.v0 import autotools as autotools_mod
from debmagic.v0 import dh as dh_mod

autotools = autotools_mod.Preset()
dh = dh_mod.Preset()

pkg = package(
    preset=[dh, autotools],
    maint_options="hardening=+all",
)

@pkg.stage
def configure(build: Build):
    autotools_mod.autoreconf(build)
    autotools.configure(
        build,
        ["--enable-something"],
    )


@dh.override
def dh_installgsettings(build: Build):
    print("test dh override works :)")
    build.cmd("dh_installgsettings")


@pkg.custom_function
def something_custom(some_param: int, another_param: str = "some default"):
    print(f"you called {some_param=} {another_param=}")


pkg.pack()
```

## Developing

Prerequisites:

- Debian >= trixie, either roll your own environment or to get started faster use the [devcontainer](https://containers.dev/)
- Python >= 3.12
- [UV](https://docs.astral.sh/uv/)

Setup:

```bash
uv sync
uv run pre-commit install
```

Build the documentation

```bash
# oneshot build
uv run sphinx-build docs docs/_build
# continous serving
uv run sphinx-autobuild docs docs/_build
```