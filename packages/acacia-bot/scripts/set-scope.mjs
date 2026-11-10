// Moves @acacia-rs/bot (name and napi.packageName) to another npm scope for a release: `node scripts/set-scope.mjs @x`.
import { readFileSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const scope = process.argv[2]
if (!/^@[a-z0-9][a-z0-9._-]*$/.test(scope ?? '')) {
  console.error(`usage: set-scope.mjs @scope (got ${scope})`)
  process.exit(1)
}
const path = join(dirname(fileURLToPath(import.meta.url)), '../package.json')
const pkg = JSON.parse(readFileSync(path, 'utf8'))
const rescope = (name) => name.replace(/^@[^/]+\//, `${scope}/`)
pkg.name = rescope(pkg.name)
pkg.napi.packageName = rescope(pkg.napi.packageName)
writeFileSync(path, `${JSON.stringify(pkg, null, 2)}\n`)
console.log(`${pkg.name} (platform packages ${pkg.napi.packageName}-*)`)
