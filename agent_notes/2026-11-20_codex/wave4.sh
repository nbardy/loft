#!/bin/bash
# Wave x4: after wave x3 and the token study. std1 design (Astra) -> std1a restructure (alone) -> std port lanes + port1 + stdaot1 (parallel, max 5) -> ix4.
D=$(cd "$(dirname "$0")" && pwd)
caffeinate -i -w "$$" &
until [ -e "$D/wave3.finished" ] && [ -s "$D/tok1.done" ]; do sleep 120; done
run() { "$D/run.sh" "$1" "$2" "$3" ~/git/loft; }
NOTE="Std rule: a lib function must replace a recurring, sufficiently complicated block of ≥3× its call's tokens. NEVER add renamed wrappers for standard patterns (map, await, filter, fold) or one-or-two-call helpers. Code lives in the stdlib/ tree (design/std1.md); forking is copy-paste."
run std1 gpt-6.2-astra xhigh
[ -s "$D/std1a.brief.md" ] && run std1a gpt-6.2-sol high
L="port1 stdaot1"; for l in $(cat "$D/std1.lanes" 2>/dev/null); do L="$L $l"; done
for l in $L; do grep -q "NEVER add renamed" "$D/$l.brief.md" || printf '\n%s\nBase: start from origin/main AFTER the std1a restructure (stdlib/ tree), not the old flat lib/.\n' "$NOTE" >> "$D/$l.brief.md"; done
pids=(); n=0; for l in $L; do run $l gpt-6.2-sol high & pids+=($!); n=$((n+1)); sleep 20; [ $n -ge 5 ] && { wait "${pids[@]}"; pids=(); n=0; }; done; wait "${pids[@]}"
sed -e "s/w1–w4/the wave x4 lanes ($L)/; s#loft/w1..w4#the branches loft/<lane> for each of: $L#; s#lanes/w\*.md#lanes/<lane>.md#; s#w\*.brief.md#<lane>.brief.md#; s#loft/intw#loft/intx4#; s#out/meta/iw#out/meta/ix4#g" "$D/iw.brief.md" > "$D/ix4.brief.md"
printf '\nThe std1a restructure is already on main. Port lanes and port1 queue look pairs in out/meta/<lane>/approval/, and the lead approves them via agent_notes/2026-11-20_v1/APPROVALS.md. New std modules and showcase scenes have no frozen reference: merge them once they build and pass Twin and Truth with existing pixels unchanged. The token-study lanes (tok1a..c) are superseded by std1 unless std1.md says otherwise.\n' >> "$D/ix4.brief.md"
run ix4 gpt-6.2-astra high
echo done > "$D/wave4.finished"
