#!/usr/bin/env bash
# Create temporary git repos that exercise rebase scenarios for manual / CI testing.
set -euo pipefail

MARKER_FILE=".gitru-rebase-fixtures"

if [[ $# -gt 0 ]]; then
  ROOT="$1"
  if [[ -z "$ROOT" || "$ROOT" == "/" ]]; then
    echo "Refusing to use empty or root path as fixture root" >&2
    exit 1
  fi
  if [[ -e "$ROOT" && ! -f "$ROOT/$MARKER_FILE" ]]; then
    echo "Refusing to delete '$ROOT' (missing $MARKER_FILE marker). Pass a fresh path or an existing fixture root." >&2
    exit 1
  fi
  rm -rf -- "$ROOT"
else
  ROOT="$(mktemp -d "${TMPDIR:-/tmp}/gitru-rebase-fixtures.XXXXXX")"
fi

mkdir -p -- "$ROOT"
touch -- "$ROOT/$MARKER_FILE"

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

expect_rebase_paused() {
  local dir="$1"
  local label="$2"
  if [[ ! -d "$dir/.git/rebase-merge" && ! -d "$dir/.git/rebase-apply" ]]; then
    echo "Expected paused rebase in $label ($dir), but no rebase state found" >&2
    exit 1
  fi
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
set +e
git -C "$CONFLICT" rebase main
set -e
expect_rebase_paused "$CONFLICT" "conflict-rebase"
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
set +e
git -C "$INTERACTIVE" rebase -i main
set -e
unset GIT_SEQUENCE_EDITOR
expect_rebase_paused "$INTERACTIVE" "interactive-pause"
echo "Fixture ready (interactive edit pause): $INTERACTIVE"

echo ""
echo "All fixtures under: $ROOT"
