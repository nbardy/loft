#!/bin/bash
# Run one codex step with a wall-clock limit and one retry: run.sh <name> <model> <effort> <dir>. Resumable.
n=$1; m=$2; e=$3; w=$4; D=$(cd "$(dirname "$0")" && pwd)
[ -s "$D/$n.done" ] && exit 0
case $m in *astra*) lim=5400;; *) lim=3600;; esac   # 90 min Astra, 60 min Sol
export RUSTC_WRAPPER=sccache CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2
for try in 1 2; do
  perl -e 'alarm shift; exec @ARGV' $lim codex exec -m "$m" --dangerously-bypass-approvals-and-sandbox -C "$w" \
    -c model_reasoning_effort="$e" -o "$D/$n.final.md" "$(cat "$D/$n.brief.md")" < /dev/null >> "$D/$n.log" 2>&1
  rc=$?; [ $rc -eq 0 ] && break
  echo "run.sh: $n try $try exited $rc; retrying (the brief continues from the workspace)" >> "$D/$n.log"
done
echo $rc > "$D/$n.done"
