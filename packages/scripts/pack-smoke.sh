#!/usr/bin/env bash
# Packs @acacia-rs/bot and @acacia-rs/mcp (built first: npm run build:ts in each), installs both tarballs into an
# empty project and loads them as a user would. The tarballs carry no native binary (the platform packages do),
# so this also checks that the package loads without one.
set -euo pipefail
packages=$(cd "$(dirname "$0")/.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
for pkg in acacia-bot acacia-mcp; do
  (cd "$packages/$pkg" && npm pack --silent --pack-destination "$work" >/dev/null)
done
cd "$work"
npm init -y >/dev/null
npm install --silent --no-audit --no-fund ./acacia-rs-bot-*.tgz ./acacia-rs-mcp-*.tgz
node --input-type=module -e "
const bot = await import('@acacia-rs/bot')
const mcp = await import('@acacia-rs/mcp')
for (const name of ['createBot', 'signIn', 'WsTransport', 'NativeTransport', 'goals']) if (!bot[name]) throw new Error('@acacia-rs/bot lacks ' + name)
if (mcp.tools().length < 10) throw new Error('@acacia-rs/mcp lists ' + mcp.tools().length + ' tools')
"
node --input-type=commonjs -e "if (typeof require('@acacia-rs/bot').createBot !== 'function') process.exit(1)"
npx --no-install acacia-mcp --help 2>&1 | grep -q '^usage: acacia-mcp'
echo "pack-smoke: ok ($(ls ./*.tgz | xargs -n1 basename | tr '\n' ' '))"
