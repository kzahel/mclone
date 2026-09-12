#!/usr/bin/env bash
set -euo pipefail
release_tools=$(mktemp -d)
trap 'rm -rf "$release_tools"' EXIT
version=8.30.1
archive="gitleaks_${version}_linux_x64.tar.gz"
gh release download "v$version" --repo gitleaks/gitleaks --pattern "$archive" --pattern "gitleaks_${version}_checksums.txt" --dir "$release_tools"
(cd "$release_tools" && awk -v name="$archive" '$2 == name' "gitleaks_${version}_checksums.txt" | sha256sum -c -)
tar -xzf "$release_tools/$archive" -C "$release_tools"
"$release_tools/gitleaks" git --log-opts=--all --redact --no-banner .
