#!/usr/bin/env bash
# Run the interface in a browser against the built-in simulator.
#
# No Windows and no audio engine required: the simulator produces moving meters
# so layout and behaviour can be worked on anywhere. Append ?demo=1 to the URL
# to start with a session already running.
set -euo pipefail
cd "$(dirname "$0")/../apps/ui"
[ -d node_modules ] || npm install
exec npm run dev
