# Making releases

All Python and Rust package versions are kept in lockstep via
[`bump-my-version`](https://callowayproject.github.io/bump-my-version/)
(configured in [`.bumpversion.toml`](../../.bumpversion.toml)):

```shell
uv run bump-my-version show-bump          # current bump graph
uv run bump-my-version bump pre_n -n      # preview next pre-release number
uv run bump-my-version bump pre_n         # apply: 0.0.1-alpha.1 → 0.0.1-alpha.2
```

Bumpable parts: `major`, `minor`, `patch`, `pre_l` (`alpha` → `beta` → `rc` →
final/stable), and `pre_n` (pre-release counter). Real bumps require a clean
git working tree; use `-n` / `--dry-run` (with `--allow-dirty` if needed) to
preview without writing.

The bump updates cargo/pyproject versions, promotes Keep-a-Changelog sections in
each package changelog, and prepends a new [`debian/changelog`](../../debian/changelog)
entry via a pre-commit hook.

Write upcoming release notes under the `[Unreleased]` heading in each package
changelog before bumping. The bump inserts `## [<new_version>] - <date>` directly
below `[Unreleased]`, so those notes become the release notes for the new version.

Then commit, tag, and push. A `v*` tag (e.g. `v0.0.1-alpha.2`) triggers the
[release workflow](../../.github/workflows/release.yaml), which runs
[CI](../../.github/workflows/ci.yaml) first and, on success, builds `debmagic`
(cli) and publishes it to PyPI via Trusted Publishing.

```shell
git push
git push origin v$(uv run bump-my-version show current_version)
```
