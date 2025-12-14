# Contribute

This section covers documentation relevant to developing and maintaining the debmagic
codebase, and some guidelines for how you can contribute.

## Getting started with development

Prerequisites:

- Debian >= trixie, either roll your own environment or to get started faster use the [devcontainer](https://containers.dev/)
- Python >= 3.12
- [UV](https://docs.astral.sh/uv/)

Setup:

```shell
uv sync
uv run pre-commit install
```

Build the documentation

```shell
# oneshot build
uv run sphinx-build docs docs/_build
# continous serving
uv run sphinx-autobuild docs docs/_build
```

```{toctree}

```
