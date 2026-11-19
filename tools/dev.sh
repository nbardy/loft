#!/usr/bin/env bash
# The Rust edit loop (plan 2026-11-19 §6, after Keel's keel/scripts/dev.sh):
#   tools/dev.sh check|lint|test|build|run CRATE [--fast|--release|--dev] [cargo args] [-- program/test args]
# Pinned toolchain (rust/rust-toolchain.toml), --locked, rust/target only, and a machine-wide compile lock shared with
# Keel: check/lint/test take LIGHT (4 jobs, niced), build/run take HEAVY (full -j). A waiter prints the holder and waits.
#   lint   single-crate Clippy with the gate's flags (tools/lib/gate_flags.sh), so it shares units with check.sh full
#   test   cargo-nextest (rust/.config/nextest.toml); `test CRATE --doc` runs the doctests with cargo test
#   run    builds under HEAVY (default --fast = profile lab), releases the lock, then execs the program at nice +15:
#          a long run never holds the compile lock (Keel 2026-11-02: a 70-minute batch stalled every lane)
#   --fast = --profile lab (release code, no thin LTO; rust/Cargo.toml says why)
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd -P)"
usage() { echo 'Usage: tools/dev.sh check|lint|test|build|run CRATE [--fast|--release|--dev] [cargo args] [-- args]' >&2; exit 2; }
[[ $# -ge 2 ]] || usage
command_name="$1" crate="$2"
shift 2
case "$command_name" in
  check|lint|build) profile=dev ;;
  test) profile="" ;;
  run) profile=lab ;;
  *) usage ;;
esac
# Two workspaces while the old tree stays (meta plan 2026-11-20 §5): the meta engine's crates/ at the root, with its
# own target/, and the legacy rust/ workspace. A crate name picks its workspace; the meta one wins a clash.
if [[ -f "$root/crates/$crate/Cargo.toml" ]]; then
  ws="$root"
  export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$root/target}"
elif [[ -f "$root/rust/crates/$crate/Cargo.toml" ]]; then
  ws="$root/rust"
else
  echo "dev.sh: no crate crates/$crate or rust/crates/$crate" >&2; exit 2
fi

# Cargo's arguments before the first `--` (the profile shorthands rewritten), the program's after it, untouched.
cargo_args=() program_args=() doc=0
while [[ $# -gt 0 && "$1" != -- ]]; do
  case "$1" in
    --fast) profile=lab ;;
    --dev) profile=dev ;;
    --release) profile=release ;;
    --profile) profile="${2:?dev.sh: --profile needs a name}"; shift ;;
    --profile=*) profile="${1#--profile=}" ;;
    --doc) doc=1 ;;
    *) cargo_args+=("$1") ;;
  esac
  shift
done
[[ $# -eq 0 ]] || { shift; program_args=("$@"); }

source "$root/tools/lib/cache-env.sh"
source "$root/tools/lib/gate_flags.sh"
source "$root/tools/lib/build_gate.sh"
cd "$ws"
toolchain="$(sed -n 's/^channel = "\([^"]*\)"/\1/p' rust-toolchain.toml)"
cargo=(rustup run "$toolchain" cargo)
case "$command_name" in
  check|lint|test) keel_gate_acquire_light "loft dev.sh $command_name $crate ($root)" ;;
  build|run) keel_gate_acquire "loft dev.sh $command_name $crate ($root)" ;;
esac

# One argument list for every command.
args=(-p "$crate")
# nextest spells Cargo's --profile as --cargo-profile (its own --profile picks a nextest.toml profile).
case "$command_name:$profile" in
  test:) ;;
  test:*) args+=(--cargo-profile "$profile") ;;
  *) args+=(--profile "$profile") ;;
esac
args+=(${cargo_args[@]+"${cargo_args[@]}"})
tail=()
[[ ${#program_args[@]} -eq 0 ]] || tail=(-- "${program_args[@]}")

case "$command_name" in
  check|build) exec "${cargo[@]}" "$command_name" --locked "${args[@]}" ${tail[@]+"${tail[@]}"} ;;
  lint) exec "${cargo[@]}" clippy "${LOFT_GATE_CLIPPY_ARGS[@]}" "${args[@]}" -- "${LOFT_GATE_LINT_ARGS[@]}" ;;
  test)
    [[ "$doc" == 0 ]] || exec "${cargo[@]}" test "${LOFT_GATE_DOCTEST_ARGS[@]}" "${args[@]}" ${tail[@]+"${tail[@]}"}
    exec "${cargo[@]}" nextest run "${LOFT_GATE_TEST_ARGS[@]}" "${args[@]}" ${tail[@]+"${tail[@]}"} ;;
  run)
    # The executable comes from Cargo's own artifact messages, never a guessed path. More than one binary needs --bin.
    executable="$("${cargo[@]}" build --locked "${args[@]}" --message-format=json-render-diagnostics | perl -MJSON::PP -ne '
      my $m = eval { decode_json($_) } or next;
      push @x, $m->{executable} if ($m->{reason} // "") eq "compiler-artifact" && defined $m->{executable};
      END { @x == 1 or die "dev.sh run: expected one executable, Cargo built " . scalar(@x) . "; name one with --bin\n"; print $x[0] }')"
    keel_gate_release
    echo "dev.sh run: $executable (compile lock released; nice +15)" >&2
    exec nice -n 15 "$executable" ${program_args[@]+"${program_args[@]}"} ;;
esac
