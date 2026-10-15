#!/bin/sh

set -eu
cd "$(dirname "$0")"

command -v wasm-pack >/dev/null || {
	echo "wasm-pack missing." >&2
	echo "fix: cargo binstall wasm-pack" >&2
	exit 1
}

MODE=--dev
[ "${1:-}" = "--release" ] && MODE=--release

wasm-pack build ext $MODE \
	--target web \
	--out-dir ../extension/pkg \
	--out-name nommetric \
	--no-typescript \
	--no-pack

echo "built $(du -h extension/pkg/nommetric_bg.wasm | cut -f1)  (${MODE#--})"
