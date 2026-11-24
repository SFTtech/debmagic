#!/bin/sh
# Prepare a fresh git worktree for an agent that implements lint Rules.
#
#   scripts/agent_prep.sh <base>      # base: the commit or branch the batch builds on
#
# A worktree an agent starts in can sit on an old `main` that has no lint code. This puts it on
# <base> (only when that cannot lose work), seeds the build cache from the main checkout with a
# reflink copy (btrfs: instant, no recompile), and prints what to run next.
set -eu

base=${1:?usage: scripts/agent_prep.sh <base-commit-or-branch>}
target=$(git rev-parse --verify --quiet "$base^{commit}") || {
    echo "unknown commit: $base" >&2
    exit 2
}
head=$(git rev-parse HEAD)

if git merge-base --is-ancestor "$target" "$head"; then
    echo "HEAD already contains $base"
else
    if [ -n "$(git status --porcelain --untracked-files=no)" ]; then
        echo "refusing to move HEAD: the working tree has changes" >&2
        exit 1
    fi
    # Moving HEAD is safe when every commit of this worktree is in the new base or on a branch.
    if [ -n "$(git rev-list "$target..$head" | head -n 1)" ] && [ -z "$(git branch -a --contains "$head")" ]; then
        echo "refusing to move HEAD: $(git rev-parse --short "$head") has commits on no branch and not in $base" >&2
        exit 1
    fi
    git reset --hard --quiet "$target"
    echo "moved HEAD to $(git rev-parse --short "$target")"
fi

main=$(git worktree list --porcelain | awk '/^worktree /{print $2; exit}')
here=$(git rev-parse --show-toplevel)
if [ "$main" != "$here" ] && [ -d "$main/target/debug" ] && [ ! -d target/debug ]; then
    mkdir -p target
    cp -a --reflink=auto "$main/target/debug" target/debug
    echo "seeded target/debug from $main"
fi

git log --oneline -1
echo "next: export CARGO_BUILD_JOBS=8; just lintian-tag <tag>"
