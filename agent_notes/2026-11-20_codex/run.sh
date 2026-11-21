#!/bin/bash
# Run one codex step with a wall-clock limit and one retry: run.sh <name> <model> <effort> <dir>. Resumable.
n=$1; m=$2; e=$3; w=$4; D=$(cd "$(dirname "$0")" && pwd)
[ -s "$D/$n.done" ] && exit 0
# A brief may set its own effort with a line "Effort: low|medium|high|xhigh" (overrides the caller).
o=$(grep -m1 -oE '^Effort: (low|medium|high|xhigh)' "$D/$n.brief.md" | awk '{print $2}'); [ -n "$o" ] && e=$o
case $m in *astra*) lim=${LIM:-10800};; *) lim=${LIM:-5400};; esac   # 3 h Astra, 90 min Sol
export RUSTC_WRAPPER=sccache CARGO_INCREMENTAL=0
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$HOME/.local/bin:$PATH"
# Polite resource guard (no ps/pgrep/lsof — avoids macOS 15.5 proc_find kernel lock convoy).
freepct() { memory_pressure 2>/dev/null | awk -F": " '/free percentage/{gsub("%","",$2); print int($2)}'; }
# Lock-file step slots (no process-table scans). Atomic create with noclobber; dead owner PIDs are reclaimed via kill -0.
S="$D/.slots"; mkdir -p "$S"; slot=""
while [ -z "$slot" ]; do
  for i in $(seq 1 "${LOFT_MAX_STEPS:-6}"); do
    if ( set -C; echo $$ > "$S/$i" ) 2>/dev/null; then slot="$S/$i"; break; fi
    o=$(cat "$S/$i" 2>/dev/null); if [ -n "$o" ] && ! kill -0 "$o" 2>/dev/null; then rm -f "$S/$i"; fi
  done
  [ -z "$slot" ] && sleep 30
done
trap 'rm -f "$slot"' EXIT
while [ "${f:=$(freepct)}" -lt 25 ] 2>/dev/null; do sleep 60; f=$(freepct); done
f=$(freepct)
if [ "${f:-0}" -lt 35 ]; then
  export CARGO_BUILD_JOBS=1
else
  export CARGO_BUILD_JOBS=2
fi

base_note=""
if [ -n "${LOFT_WAVE_BASE:-}" ]; then
  base_note="
## BASE BRANCH FOR THIS WAVE (IMPORTANT)
Create your Cambium worktree from \`${LOFT_WAVE_BASE}\` (i.e. \`git cambium add -b ... -ephemeral ... ${LOFT_WAVE_BASE}\`), NOT an older \`origin/main\` commit, so you build directly on top of the latest unified v15_exec runtime.
"
fi

compile_rule="## MANDATORY COMPILE DISCIPLINE (STRICT — DO NOT WASTE TIME COMPILING)
Compilation is the #1 bottleneck on this shared 24 GB Mac. Do NOT compile in small edit-compile loops!
1. **Read & design first:** Read ALL relevant files, structs, traits, and call sites upfront before editing.
2. **Write ALL code before your first compile:** Implement the ENTIRE feature/fix across ALL files in one big coherent pass. Reason about Rust types, borrows, imports, visibility, and trait bounds directly from source — NEVER use \`cargo check\` as a scratchpad after small edits.
3. **Target 2–3 compiles total for the entire lane:**
   - Compile #1: After ALL code is written, run ONE \`CARGO_BUILD_JOBS=2 tools/dev.sh check <smallest-crate>\`. Read EVERY error it reports and fix ALL of them in one batch before compiling again.
   - Compile #2: Run ONE focused \`tools/dev.sh test <crate> ...\` for what you changed.
   - Compile #3 (at the very end only): ONE final verification / release build, then measure once under \`tools/quiet_gpu.sh\` or \`tools/gpu_lock.sh\`.
4. **Never** run \`cargo build --release\` during iteration, **never** run concurrent cargo commands, and ALWAYS compile via \`tools/dev.sh\` (shared compile lock). Do NOT poll with \`ps\`, \`pgrep\`, or \`lsof\`.
${base_note}
---

"

prompt="${compile_rule}$(cat "$D/$n.brief.md")"

for try in 1 2; do
  [ "${LOFT_SKIP_QUIET_GPU:-0}" = "1" ] || "$D/../../tools/quiet_gpu.sh" --wait || exit $?
  # Background QoS (E-cores + throttled I/O) + nice +15 so foreground apps stay responsive.
  taskpolicy -b nice -n 15 perl -e 'alarm shift; exec @ARGV' $lim codex exec -m "$m" --dangerously-bypass-approvals-and-sandbox -C "$w" \
    -c model_reasoning_effort="$e" -o "$D/$n.final.md" "$prompt" < /dev/null >> "$D/$n.log" 2>&1
  rc=$?; [ $rc -eq 0 ] && break
  echo "run.sh: $n try $try exited $rc; retrying (the brief continues from the workspace)" >> "$D/$n.log"
done
echo $rc > "$D/$n.done"
