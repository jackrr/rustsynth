#!/usr/bin/env bash
# Creates a new git worktree at ./worktrees/[worktree-name] on a branch of the same name.
set -euo pipefail

if [ $# -ne 1 ]; then
    echo "Usage: $0 <worktree-name>" >&2
    exit 1
fi

name="$1"
repo_root="$(git rev-parse --show-toplevel)"
worktree_path="$repo_root/worktrees/$name"

if [ -e "$worktree_path" ]; then
    echo "Error: $worktree_path already exists" >&2
    exit 1
fi

mkdir -p "$repo_root/worktrees"

if git -C "$repo_root" show-ref --verify --quiet "refs/heads/$name"; then
    git -C "$repo_root" worktree add "$worktree_path" "$name"
else
    git -C "$repo_root" worktree add -b "$name" "$worktree_path"
fi

echo "Worktree created at $worktree_path"
