#!/bin/bash
# Wave x4: after wave x3. The token-study std lanes (tok1a..c) + flagship port + std AOT/pipeline cache, then one integrator.
D=$(cd "$(dirname "$0")" && pwd)
caffeinate -i -w "$$" &
until [ -e "$D/wave3.finished" ] && [ -s "$D/tok1.done" ]; do sleep 120; done
run() { "$D/run.sh" "$1" "$2" "$3" ~/git/loft; }
NOTE="Std rule: a lib function must replace a recurring, sufficiently complicated block of ≥3× its call's tokens. NEVER add renamed wrappers for standard patterns (map, await, filter, fold) or one-or-two-call helpers. See OPEN.md 'Std library rule and anti-patterns'."
L="port1 stdaot1"; for l in $(cat "$D/tok1.lanes" 2>/dev/null); do grep -q "NEVER add renamed" "$D/$l.brief.md" || printf '\n%s\n' "$NOTE" >> "$D/$l.brief.md"; L="$L $l"; done
pids=(); for l in $L; do run $l gpt-6.2-sol high & pids+=($!); sleep 20; done; wait "${pids[@]}"
sed -e "s/w1–w4/the wave x4 lanes ($L)/; s#loft/w1..w4#loft/{$(echo $L | tr ' ' ',')}#; s#lanes/w\*.md#lanes/<lane>.md#; s#w\*.brief.md#<lane>.brief.md#; s#loft/intw#loft/intx4#; s#out/meta/iw#out/meta/ix4#g" "$D/iw.brief.md" > "$D/ix4.brief.md"
printf '\nEffort: high is implied. port1 queues look pairs in out/meta/port1/approval/: the lead approves via agent_notes/2026-11-20_v1/APPROVALS.md. Merge port1 scenes that build and pass Twin and Truth even before approval, since new scenes have no frozen reference.\n' >> "$D/ix4.brief.md"
run ix4 gpt-6.2-astra high
echo done > "$D/wave4.finished"
