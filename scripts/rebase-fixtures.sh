#!/usr/bin/env bash
# Create temporary git repos that exercise rebase scenarios for manual / CI testing.
set -euo pipefail

ROOT="${1:-/tmp/gitru-rebase-fixtures}"
rm -rf "$ROOT"
mkdir -p "$ROOT"

make_base() {
  local dir="$1"
  mkdir -p "$dir"
  git -C "$dir" init -q
  git -C "$dir" config user.name "Gitru Fixture"
  git -C "$dir" config user.email "fixture@gitru.local"
  git -C "$dir" checkout -q -b main
  echo "base" >"$dir/file.txt"
  git -C "$dir" add .
  git -C "$dir" commit -q -m "base"
}

# 1) Clean rebase (feature ahead of diverged main)
CLEAN="$ROOT/clean-rebase"
make_base "$CLEAN"
git -C "$CLEAN" checkout -q -b feature
echo a >"$CLEAN/a.txt" && git -C "$CLEAN" add . && git -C "$CLEAN" commit -q -m "feat a"
echo c >"$CLEAN/c.txt" && git -C "$CLEAN" add . && git -C "$CLEAN" commit -q -m "feat c"
git -C "$CLEAN" checkout -q main
echo b >"$CLEAN/b.txt" && git -C "$CLEAN" add . && git -C "$CLEAN" commit -q -m "feat b"
git -C "$CLEAN" checkout -q feature
echo "Fixture ready: $CLEAN (run: git -C $CLEAN rebase main)"

# 2) Conflict pause
CONFLICT="$ROOT/conflict-rebase"
make_base "$CONFLICT"
git -C "$CONFLICT" checkout -q -b feature
echo feature >"$CONFLICT/conflict.txt"
git -C "$CONFLICT" add . && git -C "$CONFLICT" commit -q -m "feature conflict"
git -C "$CONFLICT" checkout -q main
echo main >"$CONFLICT/conflict.txt"
git -C "$CONFLICT" add . && git -C "$CONFLICT" commit -q -m "main conflict"
git -C "$CONFLICT" checkout -q feature
git -C "$CONFLICT" rebase main || true
echo "Fixture ready (paused): $CONFLICT"

# 3) Interactive pause via sequence editor
INTERACTIVE="$ROOT/interactive-pause"
make_base "$INTERACTIVE"
git -C "$INTERACTIVE" checkout -q -b feature
echo a >"$INTERACTIVE/a.txt" && git -C "$INTERACTIVE" add . && git -C "$INTERACTIVE" commit -q -m "feat a"
echo b >"$INTERACTIVE/b.txt" && git -C "$INTERACTIVE" add . && git -C "$INTERACTIVE" commit -q -m "feat b"
git -C "$INTERACTIVE" checkout -q main
echo m >"$INTERACTIVE/m.txt" && git -C "$INTERACTIVE" add . && git -C "$INTERACTIVE" commit -q -m "main"
git -C "$INTERACTIVE" checkout -q feature
# Break after first commit by editing todo to 'edit'
export GIT_SEQUENCE_EDITOR="sed -i.bak '1s/^pick/edit/'"
git -C "$INTERACTIVE" rebase -i main || true
unset GIT_SEQUENCE_EDITOR
echo "Fixture ready (interactive edit pause): $INTERACTIVE"

echo ""
echo "All fixtures under: $ROOT"
