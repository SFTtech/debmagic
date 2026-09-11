# Contribute

This section covers documentation relevant to developing and maintaining the debmagic
codebase, and some guidelines for how you can contribute.

## Getting started with development

Prerequisites:

- Debian >= trixie, either roll your own environment or to get started faster use the [devcontainer](https://containers.dev/)
- Rust >= edition 2024
- Python >= 3.12
- [UV](https://docs.astral.sh/uv/)

Setup:

```shell
uv sync
uv run pre-commit install
```

Build and test:

```shell
# Rust CLI and shared crate
cargo build
cargo test
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings

# Python packaging API
uv run pytest --ignore tests/integration .
uv run ruff check
uv run ty check .
```

Integration tests (end-to-end builds of real packages):

```shell
uv run pytest tests/integration
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
