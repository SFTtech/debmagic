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

# Regenerate the debmagic(1) manpage from the CLI's --help output
manpage:
    cargo build -p debmagic
    help2man --no-info -N -o debian/debmagic.1 target/debug/debmagic

# Report `debmagic lint` parity with lintian (add --run for test results, --candidates N for next Rules)
lint-parity *args:
    python3 scripts/lint_parity_report.py {{ args }}

# Everything deterministic about porting one lintian Tag: tag file, Check, API map, recipes, commands (--run: its Parity cases, --full: whole Check)
lintian-tag tag *args:
    python3 scripts/lintian_tag.py {{ tag }} {{ args }}

# Put a fresh agent worktree on BASE (when that cannot lose work) and seed target/ from the main checkout
agent-prep base:
    sh scripts/agent_prep.sh {{ base }}

# Scaffold and register the LN Rule for a lintian Tag (--code LNxxxx, --kind source|binary|both, --dry-run)
new-rule tag *args:
    python3 scripts/new_lint_rule.py {{ tag }} {{ args }}

# Rule-facing `debmagic lint` API, generated from the source (optional module filter, e.g. `just lint-api file_tree`)
lint-api *filter:
    python3 scripts/lint_api.py {{ filter }}

# Write insta snapshots of unit tests matching FILTER (no cargo-insta needed); review `git diff` afterwards
snap-accept filter:
    INSTA_UPDATE=always cargo test -p debmagic --bin debmagic {{ filter }}
    rm -f packages/debmagic/src/lint/snapshots/*.snap.new
    sed -i '/^assertion_line: /d' packages/debmagic/src/lint/snapshots/*.snap

# Ecosystem test (needs network + psql): lint key packages from the archive
lint-ecosystem *args:
    uv run scripts/lint_ecosystem.py {{ args }}

# Build the debmagic snap
snap:
    snapcraft pack

# Build debmagic itself with the docker driver
self-build *args:
    cargo run --locked -p debmagic -- build binary --driver=docker --persistent --incremental {{ args }}

# Test debmagic itself with the docker driver
self-test *args:
    cargo run --locked -p debmagic -- test --driver=docker {{ args }}
