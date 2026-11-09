#!/bin/bash
set -euo pipefail
src="$1"
shift
dest="$(dirname "$src")/_stripped_$(basename "$src")"
python3 -c "
import sys
content = open(sys.argv[1], 'rb').read().replace(b'\r\n', b'\n').replace(b'\r', b'\n')
open(sys.argv[2], 'wb').write(content)
" "$src" "$dest"
chmod +x "$dest"
exec "$dest" "$@"
