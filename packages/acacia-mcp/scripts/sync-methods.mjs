// Copies acacia-api's method table (crates/acacia-api/generated/methods.json) into src/generated, where tsc picks it
// up. The Rust side regenerates the source file (see acacia-api's crate docs).
import { copyFileSync, mkdirSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const pkg = join(dirname(fileURLToPath(import.meta.url)), '..')
mkdirSync(join(pkg, 'src/generated'), { recursive: true })
copyFileSync(join(pkg, '../../crates/acacia-api/generated/methods.json'), join(pkg, 'src/generated/methods.json'))
console.log('sync-methods: methods.json from crates/acacia-api/generated')
