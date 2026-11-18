#!/usr/bin/env bash
# Parallel lanes as ephemeral Cambium worktrees (plan 2026-11-19 §6, after Keel's keel/scripts/lane.sh):
#   tools/lane.sh new NAME BRANCH [BASE]  create ~/git/_wt/NAME on a new BRANCH from BASE (default: this checkout's
#                                         HEAD commit; commit first). Cambium clones node_modules; rust/target and
#                                         out/ start empty (.cambium.toml)
#   tools/lane.sh done NAME               cambium remove NAME, only for a clean lane under the lane root
#   tools/lane.sh check NAME              print NAME's path if `done` would accept it; changes nothing
# LOFT_LANE_ROOT moves the lane root (default ~/git/_wt).
set -euo pipefail
repo="$(cd "$(dirname "$0")/.." && pwd -P)"
lane_root="${LOFT_LANE_ROOT:-$HOME/git/_wt}"
usage() { echo 'Usage: tools/lane.sh new NAME BRANCH [BASE] | done NAME | check NAME' >&2; exit 2; }
valid_name() { [[ "$1" =~ ^[A-Za-z0-9][A-Za-z0-9._-]*$ ]] || { echo "lane: invalid name '$1'" >&2; exit 2; }; }

lane_new() {
  local name="$1" branch="$2" base path
  valid_name "$name"
  # Explicit base: without a commit Cambium bases the lane on the primary checkout's HEAD, not this worktree's
  # (Keel 2026-10-30: a lane made from keel-complete came up on main).
  base="$(git -C "$repo" rev-parse --verify "${3:-HEAD}^{commit}")"
  path="$lane_root/$name"
  [[ ! -e "$path" ]] || { echo "lane: $path already exists" >&2; exit 2; }
  mkdir -p "$lane_root"
  (cd "$repo" && git cambium add -b "$branch" -name "$name" -ephemeral "$path" "$base")
  echo "lane: $name at $path on $branch from $base"
}

# Resolve NAME to a lane this script may remove, or refuse. Containment under the lane root is the guard that
# matters: Cambium reports every workspace ephemeral, the primary checkout included (Keel 2026-10-30 review). A dirty
# lane is refused too, because cambium remove deletes its untracked and ignored files.
lane_checked() {
  local name="$1" root path
  valid_name "$name"
  root="$(cd "$lane_root" && pwd -P)"
  path="$(cd "$repo" && cambium path "$name" 2>/dev/null)" || {
    echo "lane: refusing $name: not a Cambium workspace (the primary checkout never is one; lanes come from lane.sh new)" >&2
    exit 2
  }
  path="$(cd "$path" && pwd -P)"
  [[ "$path" == "$root/$name" ]] || {
    echo "lane: refusing $name: $path is not $root/$name (only lanes made by lane.sh new are removed)" >&2
    exit 2
  }
  [[ -z "$(git -C "$path" status --porcelain)" ]] || {
    echo "lane: refusing $name: $path has uncommitted changes; commit or discard them first" >&2
    exit 2
  }
  echo "$path"
}

[[ $# -ge 1 ]] || usage
case "$1" in
  new) [[ $# -eq 3 || $# -eq 4 ]] || usage; lane_new "$2" "$3" "${4:-HEAD}" ;;
  done) [[ $# -eq 2 ]] || usage; lane_checked "$2" >/dev/null; (cd "$repo" && cambium remove "$2") ;;
  check) [[ $# -eq 2 ]] || usage; lane_checked "$2" ;;
  *) usage ;;
esac
