#!/bin/bash
# Superseded: use swarm_status.sh (file-only, no process-table scans).
exec "$(dirname "$0")/swarm_status.sh" "$@"
