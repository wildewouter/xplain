#!/bin/sh
# Build a fixture repo: sh e2e/fixture.sh <dest> [standard|empty|nogit]
#   standard: git repo, HEAD = tests/fixture/base, working tree = tests/fixture/work (*.fx renamed, so tsc ignores them)
#   empty:    git repo with one commit, no changes
#   nogit:    empty plain directory
set -e
ROOT=$(cd "$(dirname "$0")/.." && pwd)
R=$1
KIND=${2:-standard}
[ -n "$R" ] || { echo "usage: fixture.sh <dest> [standard|empty|nogit]" >&2; exit 2; }
mkdir -p "$R"
unfx() { for x in $(cd "$R" && find . -name '*.fx'); do mv "$R/$x" "$R/${x%.fx}"; done; }
commit() {
	git -C "$R" -c init.defaultBranch=main init -q
	git -C "$R" add -A
	git -C "$R" -c user.email=a@b.c -c user.name=x -c commit.gpgsign=false commit -q --allow-empty -m init
}
case "$KIND" in
standard)
	cp -R "$ROOT/tests/fixture/base/." "$R/"
	unfx
	commit
	cp -R "$ROOT/tests/fixture/work/." "$R/"
	unfx
	;;
empty) commit ;;
nogit) ;;
*) echo "fixture.sh: unknown fixture $KIND" >&2; exit 2 ;;
esac
