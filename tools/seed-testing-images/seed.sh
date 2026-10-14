#!/usr/bin/env sh
# Seed the Testing Stash (:9998) with images for 012; see seed.py. Testing only.
# Usage: tools/seed-testing-images/seed.sh --creds <Testing creds file>
exec python3 "$(dirname "$0")/seed.py" "$@"
