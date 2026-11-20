#!/bin/bash
# Wait up to ~9 min for <lane>.done. Prints DONE <exit> or WAITING.
dir="$(cd "$(dirname "$0")" && pwd)"
for i in $(seq 54); do [ -e "$dir/$1.done" ] && { echo "DONE $(cat "$dir/$1.done")"; exit 0; }; sleep 10; done
echo "WAITING log_bytes=$(wc -c < "$dir/$1.log")"
