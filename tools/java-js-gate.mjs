// The Node package's Java gate, run by tools/java-js-gate.sh: createBot, chat, a goto and a close, printing
// `GATE ok` or `GATE <reason>`. `node tools/java-js-gate.mjs <host> <port> <username> [profilesFolder]`; a profiles
// folder signs in with Microsoft from its cached tokens and never asks for a device code.
import { createBot, goals } from '../packages/acacia-bot/dist/index.js'

const [host, port, username, profilesFolder] = process.argv.slice(2)
const auth = profilesFolder ? { auth: 'microsoft', profilesFolder } : {}
const noCode = () => {
  console.log(`GATE ${username} is not signed in for Java in ${profilesFolder}`)
  process.exit(3)
}

const bot = await createBot({ host, port: Number(port), username, edition: 'java', ...auth, onMsaCode: noCode })
let reason = 'ok'
try {
  await bot.ready()
  const from = bot.entity.position
  console.log(`joined as ${bot.entity.username ?? username} at ${from.x.toFixed(1)} ${from.y.toFixed(1)} ${from.z.toFixed(1)}`)
  // A server applies the login's chat session on its main thread, which lags on a fresh server's first join, and
  // refuses signed chat until then (vanilla too): docs/java-edition.md, Joining.
  await new Promise((resolve) => setTimeout(resolve, 2000))
  await bot.chat('gate hello')
  await bot.goto(goals.xz(Math.floor(from.x) + 6, Math.floor(from.z) + 3), { signal: AbortSignal.timeout(30_000) })
  const to = bot.entity.position
  const moved = Math.hypot(to.x - from.x, to.z - from.z)
  if (moved < 5) reason = `goto moved only ${moved.toFixed(2)}`
} catch (e) {
  reason = `${e.name}: ${e.message}`
}
await bot.close()
console.log(`GATE ${reason}`)
process.exit(reason === 'ok' ? 0 : 1)
