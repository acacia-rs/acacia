#!/usr/bin/env bash
# Fetches the vanilla resource pack subset acacia-render needs from Mojang/bedrock-samples into assets/vanilla.
# BDS ships no textures, so this is the asset source. The tag must match the block palette (registry/mod.rs).
# usage: tools/fetch-vanilla-pack.sh [tag]
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
tag=${1:-v1.26.50.4}
dest="$root/assets/vanilla"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

git clone -q --depth 1 --branch "$tag" --filter=blob:none --sparse https://github.com/Mojang/bedrock-samples.git "$tmp/repo"
git -C "$tmp/repo" sparse-checkout set --no-cone \
  resource_pack/blocks.json resource_pack/biomes_client.json \
  resource_pack/textures/terrain_texture.json resource_pack/textures/flipbook_textures.json \
  resource_pack/textures/blocks/ resource_pack/textures/colormap/ resource_pack/textures/environment/ \
  resource_pack/entity/ resource_pack/models/ resource_pack/textures/entity/ \
  resource_pack/render_controllers/ resource_pack/animations/ resource_pack/animation_controllers/
rm -rf "$dest"
mkdir -p "$dest"
cp -r "$tmp/repo/resource_pack/." "$dest/"
echo "$tag" >"$dest/VERSION"
echo "vanilla pack $tag -> $dest ($(find "$dest" -type f | wc -l) files, $(du -sh "$dest" | cut -f1))"
