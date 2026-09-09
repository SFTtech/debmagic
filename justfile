default:
    @just --list

# Build the whole Rust workspace
build:
    cargo build

# Run Rust unit tests
test:
    cargo test
    uv run pytest --ignore tests/integration .

# Format Rust and Python code
fmt:
    cargo fmt --all
    uv run ruff format

# Check formatting without writing
fmt-check:
    cargo fmt --all --check
    uv run ruff format --check

# Lint Rust and Python code
lint:
    cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
    uv run ruff check

# Typecheck Python
typecheck:
    uv run ty check .

# Build the Sphinx documentation
docs:
    uv run sphinx-build docs docs/_build

# Live-preview the documentation
docs-live:
    uv run sphinx-autobuild docs docs/_build

# Everything CI runs (minus the self-packaging docker job)
ci: fmt-check lint typecheck test docs

# Run pre-commit hooks on all files
pre-commit:
    uv run pre-commit run --all-files

# Build debmagic itself with the docker driver
self-build *args:
    cargo run --locked -p debmagic -- build binary --driver=docker --persistent --incremental {{ args }}

# Test debmagic itself with the docker driver
self-test *args:
    cargo run --locked -p debmagic -- test --driver=docker {{ args }}
