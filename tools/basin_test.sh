#!/usr/bin/env bash
# Prepare cold prompts, or score independently generated scenes.
set -euo pipefail
export LC_ALL=C
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
loft=${LOFT_BIN:-"$root/target/release/loft"}

usage() {
  echo "Usage: tools/basin_test.sh [--prompts OUT_DIR | --check SCENE_DIR [OUT_DIR]]" >&2
  exit 2
}

absolute_dir() { (cd "$1" && pwd); }

case "${1:---prompts}" in
  --prompts)
    [[ $# -le 2 ]] || usage
    out=${2:-"$root/out/basin/prompts"}
    mkdir -p "$out"
    out=$(absolute_dir "$out")
    cd "$root"
    context=$("$loft" context)
    printf '%s\n\n%s\n' "$context" 'a clockwork beetle crossing rain puddles in ink wash' > "$out/beetle.txt"
    printf '%s\n\n%s\n' "$context" 'a glass jellyfish pulsing over a bioluminescent reef' > "$out/jellyfish.txt"
    printf '%s\n\n%s\n' "$context" 'an exhausted stone golem waking up with heavy spring follow-through' > "$out/golem.txt"
    printf '%s\n' "$out"
    ;;
  --check)
    [[ $# -ge 2 && $# -le 3 ]] || usage
    dir=$(absolute_dir "$2")
    if [[ $# == 3 ]]; then
      out=$3
    else
      mkdir -p "$root/out/basin"
      out=$(mktemp -d "$root/out/basin/check.XXXXXX")
    fi
    mkdir -p "$out"
    out=$(absolute_dir "$out")
    [[ ! -e "$out/scores.tsv" ]] || { echo "Choose a fresh output directory: $out" >&2; exit 2; }
    cd "$root"
    shopt -s nullglob
    scenes=("$dir"/*.loft)
    [[ ${#scenes[@]} -gt 0 ]] || { echo "No .loft scenes in $dir" >&2; exit 2; }

    failed=0
    captures=()
    tiles=()
    printf 'scene\tcheck\tlint\tgrammar\tprofile\tstill\n' > "$out/scores.tsv"
    for scene in "${scenes[@]}"; do
      stem=${scene##*/}
      stem=${stem%.loft}
      run="$out/$stem"
      mkdir -p "$run"
      checked=0 linted=0 grammar=0 profiled=0 rendered=0
      "$loft" check "$scene" > "$run/check.log" 2>&1 || checked=$?
      "$loft" lint --strict "$scene" > "$run/lint.log" 2>&1 || linted=$?
      uv run --script "$root/tools/check_loft_grammar.py" "$scene" > "$run/grammar.log" 2>&1 || grammar=$?
      "$root/tools/gpu_lock.sh" "$loft" profile "$scene" --size 384x384 --frames 60 --warmup 30 \
        --json "$run/profile.json" > "$run/profile.log" 2>&1 || profiled=$?
      # Keep the capture lossless until montage. Save the deliverable as WebP q95.
      tmp=$(mktemp -d "$run/capture.XXXXXX")
      "$root/tools/gpu_lock.sh" "$loft" still "$scene" "$tmp/still.png" --t 0.91 \
        > "$run/still.log" 2>&1 || rendered=$?
      if [[ $rendered == 0 ]]; then
        magick "$tmp/still.png" -quality 95 "$run/still.webp" >> "$run/still.log" 2>&1 || rendered=$?
        if [[ $rendered == 0 ]]; then
          captures+=("$tmp/still.png")
          tiles+=(-label "$stem" "$tmp/still.png")
        fi
      fi
      printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$stem" "$checked" "$linted" "$grammar" "$profiled" "$rendered" |
        tee -a "$out/scores.tsv"
      if (( checked || linted || grammar || profiled || rendered )); then failed=1; fi
    done
    if [[ ${#tiles[@]} -gt 0 ]]; then
      magick montage -font Helvetica -pointsize 15 -fill white -background '#14202c' \
        "${tiles[@]}" -tile 4x -geometry '256x256+5+8' -quality 95 "$out/contact-sheet.webp" || failed=1
      for capture in "${captures[@]}"; do
        rm -- "$capture"
        rmdir -- "${capture%/*}"
      done
    fi
    printf '%s\n' "$out"
    exit "$failed"
    ;;
  *) usage ;;
esac
