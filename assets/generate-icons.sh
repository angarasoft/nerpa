#!/usr/bin/env bash
# Regenerate the nerpa icon set and logo from assets/icon.png.
#
# Requires Python 3 with Pillow.  Everything is derived from the single
# mark file; see generate_icons.py for the list of outputs.
set -euo pipefail

cd "$(dirname "$0")"

command -v python3 >/dev/null 2>&1 || { echo "python3 is required" >&2; exit 1; }
python3 -c 'import PIL' >/dev/null 2>&1 || { echo "Pillow is required (pip install pillow)" >&2; exit 1; }

exec python3 generate_icons.py "$@"
