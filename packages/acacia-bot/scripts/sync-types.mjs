// Copies acacia-api's generated TS (crates/acacia-api/generated) into src/generated, adding the `.js` extensions
// NodeNext resolution needs. The Rust side regenerates the source files (see acacia-api's crate docs).
import { cpSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const pkg = join(dirname(fileURLToPath(import.meta.url)), '..')
const from = join(pkg, '../../crates/acacia-api/generated')
const to = join(pkg, 'src/generated')

rmSync(to, { recursive: true, force: true })
cpSync(from, to, { recursive: true, filter: (p) => statSync(p).isDirectory() || p.endsWith('.ts') })

let files = 0
const walk = (dir) => {
  for (const name of readdirSync(dir)) {
    const path = join(dir, name)
    if (statSync(path).isDirectory()) walk(path)
    else {
      const text = readFileSync(path, 'utf8').replace(/from "(\.{1,2}\/[^"]+?)(?<!\.js)"/g, 'from "$1.js"')
      writeFileSync(path, text)
      files++
    }
  }
}
walk(to)
console.log(`sync-types: ${files} files from crates/acacia-api/generated`)
