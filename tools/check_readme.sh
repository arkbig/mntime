#!/bin/sh
set -eu
# set -eux

ROOTDIR=$(cd "$(dirname "$0")"/.. && pwd)
cd "$ROOTDIR"

echo Check MSRV:
cargo_toml_msrv=$(cargo read-manifest | jq -r .rust_version)
readme_msrv=$(grep "MSRV" README.md)
echo "$readme_msrv" | grep "$cargo_toml_msrv" || (echo "'$readme_msrv' in README.md does not match '$cargo_toml_msrv' in Cargo.toml" && false)

echo Check dependencies:
cargo license --direct-deps-only --avoid-build-deps --avoid-dev-deps | awk -F ":" '{printf "|%s|%s|\n", $1, $2}' >temp.tmp
cargo license --avoid-build-deps --avoid-dev-deps | awk -F ":" '{printf "|%s|%s|\n", $1, $2}' >>temp.tmp
grep -f temp.tmp README.md | diff temp.tmp -
rm temp.tmp

echo Check content between main.rs doc comment:
awk '/\/\*!/,/\*\//' "src/main.rs" | \diff README.md - --old-line-format='> %L' --new-line-format='< %L' --unchanged-line-format='' | diff - tools/readme_main.diff
