#!/bin/sh
# Assemble the npm packages from the release tarballs:
#   npm/build.sh 0.6.0 dist/      (dist/ holds hntui-<os>-<arch>.tar.gz)
# Writes npm/out/hntui-<os>-<arch>/ (one per binary found) and npm/out/hntui/,
# whose optionalDependencies pin exactly those packages at this version.
set -eu
version=$1
dist=$2
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/.." && pwd)
out="$here/out"
rm -rf "$out"
mkdir -p "$out"

deps=""
for tarball in "$dist"/hntui-*.tar.gz; do
  suffix=$(basename "$tarball" .tar.gz)
  suffix=${suffix#hntui-}
  os=${suffix%-*}
  cpu=${suffix#*-}
  case "$os" in darwin) osname="macOS" ;; linux) osname="Linux" ;; esac
  dir="$out/hntui-$suffix"
  mkdir -p "$dir/bin"
  tar -xzf "$tarball" -C "$dir/bin"
  chmod 755 "$dir/bin/hntui"
  cat > "$dir/package.json" <<JSON
{
  "name": "@ahmd-sh/hntui-$suffix",
  "version": "$version",
  "description": "The hntui binary for $osname ($cpu). Install @ahmd-sh/hntui instead.",
  "license": "MIT",
  "repository": { "type": "git", "url": "git+https://github.com/ahmd-sh/hntui.git" },
  "publishConfig": { "access": "public" },
  "os": ["$os"],
  "cpu": ["$cpu"],
  "files": ["bin/hntui"]
}
JSON
  deps="$deps${deps:+,}\"@ahmd-sh/hntui-$suffix\": \"$version\""
done
[ -n "$deps" ] || { echo "no tarballs in $dist" >&2; exit 1; }

mkdir -p "$out/hntui"
cp -R "$here/hntui/bin" "$out/hntui/"
cp "$root/LICENSE" "$root/README.md" "$out/hntui/"
node -e '
  const fs = require("fs");
  const [, template, target, version, deps] = process.argv;
  const pkg = JSON.parse(fs.readFileSync(template, "utf8"));
  pkg.version = version;
  pkg.optionalDependencies = JSON.parse("{" + deps + "}");
  fs.writeFileSync(target, JSON.stringify(pkg, null, 2) + "\n");
' "$here/hntui/package.json" "$out/hntui/package.json" "$version" "$deps"
echo "built in $out:"
ls "$out"
