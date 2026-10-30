#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Build a deterministic source tarball for the PKGBUILD. `packaging/arch` is
# export-ignored, so the archive content does not depend on this script or on
# the checksum stored in the PKGBUILD. Fixed mtimes, ownership, and sort order
# make the SHA-256 stable across commits with identical content.
set -eu

version=0.1.0
root=$(git rev-parse --show-toplevel)
out=${1:-"$root/packaging/arch/atvv-linux-$version.tar.gz"}

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/root"

git -C "$root" archive --format=tar --prefix="atvv-linux-$version/" HEAD \
  | tar -xf - -C "$tmp/root"

tar --sort=name --mtime='@0' --owner=0 --group=0 --numeric-owner \
  -C "$tmp/root" -cf - . \
  | gzip -n >"$out"

sha256sum "$out"
