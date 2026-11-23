#!/bin/bash
# Codex usage cap hit 23 Nov ("try again at Nov 25th, 2026 9:22 PM"). Resume match1 and lean2 after it lifts, then run the
# post-lane cleanup. Never substitute a weaker model while capped: wait.
D=$(cd "$(dirname "$0")" && pwd)
until [ "$(date +%Y%m%d%H%M)" -ge 202611252130 ]; do sleep 600; done
for s in match1 lean2; do rm -f "$D/$s.done"; LOFT_MAX_STEPS=3 nohup "$D/run.sh" "$s" gpt-6.2-astra xhigh ~/git/loft < /dev/null > /dev/null 2>&1 & sleep 30; done
wait
echo "resumed $(date)" >> "$D/resume_after_cap.log"
