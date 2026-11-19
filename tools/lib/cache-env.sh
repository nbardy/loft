#!/usr/bin/env bash
# Sourced by tools/dev.sh and tools/check.sh: the pinned compiler, one admitted target dir, sccache, incremental off.
# Ported from Keel's keel/scripts/cache-env.sh (plan 2026-11-19 §6); the incidents are Keel's, on this same machine.
if [ -z "${BASH_VERSION:-}" ]; then
  echo "cache-env.sh must be sourced by bash; use tools/dev.sh" >&2
  return 2 2>/dev/null || exit 2
fi
loft_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd -P)"
export CARGO_INCREMENTAL="${CARGO_INCREMENTAL:-0}"

# Pinned compiler: bare `cargo` must be the rustup proxy so rust-toolchain.toml applies. 2026-11-01 (Keel):
# Homebrew's cargo came first on PATH and its newer Clippy failed two crates the pinned toolchain passes.
loft_rustup_bin="${CARGO_HOME:-$HOME/.cargo}/bin"
[[ -x "$loft_rustup_bin/rustup" ]] || {
  echo "cache-env.sh: no rustup at $loft_rustup_bin, so rust-toolchain.toml cannot be enforced; install rustup (https://rustup.rs)" >&2
  return 2
}
export PATH="$loft_rustup_bin:$PATH"

# Pure-shell realpath (Keel replaced a python3 spawn of ~30 ms per call): components left to right, a symlink replaced
# by its target before the next component, `..` pops the resolved parent. Works for paths that do not exist yet.
loft_realpath() {
  local path="$1" depth="${2:-0}" resolved="" part link
  [[ "$depth" -lt 40 ]] || { echo "cache-env.sh: too many symlink hops resolving $1" >&2; return 2; }
  [[ "$path" == /* ]] || path="$PWD/$path"
  local IFS=/
  local -a parts
  read -r -a parts <<< "$path"
  for part in "${parts[@]}"; do
    case "$part" in
      ''|.) ;;
      ..) resolved="${resolved%/*}" ;;
      *)
        if [[ -L "$resolved/$part" ]]; then
          link="$(readlink "$resolved/$part")"
          [[ "$link" == /* ]] || link="$resolved/$link"
          resolved="$(loft_realpath "$link" $((depth + 1)))" || return 2
        else
          resolved="$resolved/$part"
        fi
        ;;
    esac
  done
  printf '%s\n' "${resolved:-/}"
}

# One build folder per worktree. 2026-10-30 (Keel): agents exported a fresh CARGO_TARGET_DIR per task; ~100 folders
# held 64 GB, each a private full rebuild nothing reused. And a target shared between checkouts is unsafe: Cargo's
# unit hashes ignore the checkout path, so a cloned target reports the other tree's crates Fresh.
#   primary  target
#   owned    target/<sub>, only with LOFT_TARGET_OWNER=<task>; that owner deletes it at lane end
#   unowned  target/<sub> without an owner: refused
#   outside  anything else: refused
loft_admit_target() {
  local dir
  dir="$(loft_realpath "$1")" || return 2
  case "$dir" in
    "$loft_root/target") ;;
    "$loft_root/target/"*)
      [[ "${LOFT_TARGET_OWNER:-}" =~ ^[A-Za-z0-9._-]+$ ]] || {
        echo "CARGO_TARGET_DIR=$dir: per-task build folders are refused (Keel 2026-10-30: ~100 of them, 64 GB). Unset it, or set LOFT_TARGET_OWNER=<task> and delete the folder at lane end." >&2
        return 2
      }
      echo "CARGO_TARGET_DIR=$dir is owned by task '$LOFT_TARGET_OWNER', which removes it at lane end." >&2 ;;
    *)
      echo "CARGO_TARGET_DIR=$dir: Cargo outputs stay in this worktree's target (unset CARGO_TARGET_DIR); sccache shares compatible units across worktrees." >&2
      return 2 ;;
  esac
  printf '%s\n' "$dir"
}
CARGO_TARGET_DIR="$(loft_admit_target "${CARGO_TARGET_DIR:-$loft_root/target}")" || return 2
export CARGO_TARGET_DIR
export SCCACHE_CACHE_SIZE="${SCCACHE_CACHE_SIZE:-6G}"
# sccache refuses incremental units: CARGO_INCREMENTAL=1 is an explicit edit-loop opt-in that drops the wrapper.
if [[ -z "${RUSTC_WRAPPER+x}" ]]; then
  if [[ "$CARGO_INCREMENTAL" == 1 ]]; then
    export RUSTC_WRAPPER=""
  elif command -v sccache >/dev/null 2>&1; then
    export RUSTC_WRAPPER="$(command -v sccache)"
  fi
fi
if [[ "${LOFT_VERBOSE:-}" == 1 || -z "${RUSTC_WRAPPER:-}" || "$(command -v cargo)" != "$loft_rustup_bin/cargo" ]]; then
  echo "Cargo: $(command -v cargo); target: $CARGO_TARGET_DIR; incremental=$CARGO_INCREMENTAL; wrapper=${RUSTC_WRAPPER:-none (install sccache for shared reuse)}" >&2
fi
