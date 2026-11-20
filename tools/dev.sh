#!/usr/bin/env bash
# Compile and run members of the root Cargo workspace.
#   tools/dev.sh check|lint|test|build|run CRATE|--workspace [--fast|--release|--dev] [cargo args] [-- program/test args]
#   tools/dev.sh seed   warms the workspace check and lab CLI, then publishes their dependency artifacts to Cambium
# Pinned toolchain (rust-toolchain.toml), --locked, checkout-local target, and a machine-wide compile lock shared with
# Keel: check/lint/test take LIGHT (niced), build/run take HEAVY. Both default to two jobs.
#   lint   Clippy with the workspace lint flags in tools/lib/gate_flags.sh
#   test   cargo-nextest; `test CRATE --doc` runs the doctests with cargo test
#   run    builds under HEAVY (default --fast = profile lab), releases the lock, then execs the program at nice +15:
#          a long run never holds the compile lock (Keel 2026-11-02: a 70-minute batch stalled every lane)
#   --fast = --profile lab (release code, no thin LTO; Cargo.toml says why)
set -euo pipefail
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
root="$(cd "$(dirname "$0")/.." && pwd -P)"
usage() {
  echo 'Usage: tools/dev.sh check|lint|test|build|run CRATE|--workspace [--fast|--release|--dev] [cargo args] [-- args]' >&2
  echo '       tools/dev.sh seed' >&2
  exit 2
}
toolchain="$(sed -n 's/^channel = "\([^"]*\)"/\1/p' "$root/rust-toolchain.toml")"
if [[ "${1:-}" == seed ]]; then
  [[ $# -eq 1 ]] || usage
  cd "$root"
  # Cambium reads the default target tree directly, including Cargo's intermediate artifacts.
  export CARGO_TARGET_DIR="$root/target" CARGO_BUILD_BUILD_DIR="$root/target"
  export RUSTUP_TOOLCHAIN="$toolchain"
  source "$root/tools/lib/cache-env.sh"
  tools/dev.sh check --workspace
  tools/dev.sh build loft-cli --fast

  # Cambium 0.10 checks .cargo-lock. Cargo 1.99 holds .cargo-build-lock while writing intermediates.
  shopt -s nullglob
  exec perl -MFcntl=:flock -e '
    my @locks;
    for my $path (@ARGV) {
      open my $lock, "<", $path or die "dev.sh seed: $path: $!\n";
      flock $lock, LOCK_SH | LOCK_NB or die "dev.sh seed: $path: $!\n";
      push @locks, $lock;
    }
    exit(system("cambium", "seed", "refresh") == 0 ? 0 : 1);
  ' target/{*,*/*}/.cargo-build-lock
fi
[[ $# -ge 2 ]] || usage
command_name="$1" crate="$2"
shift 2
case "$command_name" in
  check|lint|build) profile=dev ;;
  test) profile="" ;;
  run) profile=lab ;;
  *) usage ;;
esac
if [[ "$crate" == --workspace ]]; then
  scope=(--workspace)
elif [[ -f "$root/crates/$crate/Cargo.toml" ]]; then
  scope=(-p "$crate")
else
  echo "dev.sh: unknown workspace crate '$crate'" >&2; exit 2
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
cd "$root"
cargo=(rustup run "$toolchain" cargo)
case "$command_name" in
  check|lint|test) keel_gate_acquire_light "loft dev.sh $command_name $crate ($root)" ;;
  build|run) keel_gate_acquire "loft dev.sh $command_name $crate ($root)" ;;
esac

# One argument list for every command.
args=("${scope[@]}")
# nextest spells Cargo's --profile as --cargo-profile (its own --profile picks a nextest.toml profile).
case "$command_name:$profile" in
  test:) ;;
  test:*) if [[ "$doc" == 1 ]]; then args+=(--profile "$profile"); else args+=(--cargo-profile "$profile"); fi ;;
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
