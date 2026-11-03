import type { ChatLine, Entity, Form, Item } from '@acacia/bot'

import type { BotHandle, ConnectOptions } from './bots.js'
import { eyesOf, nearGoal } from './entities.js'
import { botSchema, distance, fmtPos, guarded, here, kindName, ok, round, type JsonSchema, type ToolDef, type ToolResult } from './tool.js'

type Args = Record<string, unknown>

const GAME_MODES: Record<number, string> = { 0: 'survival', 1: 'creative', 2: 'adventure', 6: 'spectator' }
const at = (p: { x: number; y: number; z: number }) => ({ x: round(p.x), y: round(p.y), z: round(p.z) })
const itemText = (i: Item) => `${i.name ?? `item #${i.networkId}`} x${i.count}`

/** A hand-written tool on one bot (named by the optional `bot` arg). */
function botTool(def: Omit<ToolDef, 'run'> & { run(handle: BotHandle, args: Args, signal: AbortSignal): Promise<ToolResult> | ToolResult }): ToolDef {
  return {
    ...def,
    run: ({ bot, ...args }, { bots, signal }) => guarded(async () => def.run(await bots.get(bot as string | undefined), args, signal)),
  }
}

/** Tracked entities nearest first, without the bot itself. */
function nearby(handle: BotHandle, maxDistance: number): (Entity & { distance: number })[] {
  const me = handle.bot.entity?.position
  if (!me) return []
  return Object.values(handle.bot.entities)
    .map((e) => ({ ...e, distance: distance(me, e.position) }))
    .filter((e) => e.distance <= maxDistance && e.username !== handle.name)
    .sort((a, b) => a.distance - b.distance)
}

function formText(form: Form): string {
  const { body } = form
  const content = body.type === 'custom' ? '' : ` "${body.content}"`
  if (body.type === 'simple') return `#${form.id} "${form.title}"${content}, buttons: ${body.buttons.map((b, i) => `${i} ${b.text}`).join(', ')}`
  if (body.type === 'modal') return `#${form.id} "${form.title}"${content}, yes "${body.yes}" / no "${body.no}"`
  const inputs = body.elements.filter((e) => 'text' in e && e.type !== 'label' && e.type !== 'header')
  return `#${form.id} "${form.title}", inputs: ${inputs.map((e) => `${e.type} "${'text' in e ? e.text : ''}"`).join(', ')}`
}

const chatText = (line: ChatLine) => {
  const params = line.params.length > 0 ? ` (${line.params.join(', ')})` : ''
  return line.sender ? `<${line.sender}> ${line.plain}${params}` : `[${line.kind}] ${line.plain}${params}`
}

function status(handle: BotHandle): ToolResult {
  const { bot } = handle
  const hot = bot.hot
  if (!hot) return ok(`${handle.name} is connected but has no state yet; try again in a moment`, { bot: handle.name })
  const held = bot.heldItem
  const players = nearby(handle, Infinity)
    .filter((e) => e.username !== undefined)
    .map((e) => ({ name: e.username!, distance: e.distance, position: at(e.position) }))
  const offline = Object.values(bot.players).filter((p) => !p.entity && p.username !== handle.name).map((p) => p.username)
  const kinds = new Map<string, { kind: string; count: number; nearest: number }>()
  for (const e of nearby(handle, 32).filter((e) => e.username === undefined)) {
    const k = kinds.get(e.kind) ?? kinds.set(e.kind, { kind: e.kind, count: 0, nearest: e.distance }).get(e.kind)!
    k.count++
  }
  const forms = [...handle.forms.values()]
  const structured = {
    bot: handle.name,
    position: at(hot.position),
    health: hot.health,
    food: hot.food,
    gameMode: GAME_MODES[hot.gameMode] ?? String(hot.gameMode),
    dimension: hot.dimension,
    heldItem: held && { name: held.name, count: held.count },
    players,
    playersOutOfView: offline,
    entities: [...kinds.values()],
    openForms: forms.map((f) => f.id),
    unreadChat: handle.unread.length,
  }
  const lines = [
    `${handle.name} at ${fmtPos(hot.position)} (dimension ${hot.dimension}, ${structured.gameMode}), health ${hot.health}/${hot.maxHealth}, food ${hot.food}`,
    `holding: ${held ? itemText(held) : 'nothing'}`,
    `players in view: ${players.map((p) => `${p.name} ${p.distance} away at ${fmtPos(p.position)}`).join('; ') || 'none'}`,
    ...(offline.length ? [`players out of view: ${offline.join(', ')}`] : []),
    `entities within 32: ${structured.entities.map((k) => `${k.count} ${k.kind} (nearest ${k.nearest})`).join(', ') || 'none'}`,
    ...forms.map((f) => `open form ${formText(f)}`),
    `unread chat: ${handle.unread.length} (readChat)`,
  ]
  return ok(lines.join('\n'), structured)
}

const CONNECT_SCHEMA: JsonSchema = {
  type: 'object',
  properties: {
    host: { type: 'string', description: 'Server address; the --host default when absent.' },
    port: { type: 'integer', minimum: 1, maximum: 65535, description: 'Default --port, else 19132.' },
    username: { type: 'string', description: 'The bot name (offline auth); the --username default when absent.' },
    transport: { type: 'string', enum: ['auto', 'raknet', 'nethernet'], description: 'Default auto.' },
  },
  required: [],
}

export const COMPOSITE_TOOLS: ToolDef[] = [
  {
    name: 'connect',
    description: 'Connects a bot to a Bedrock server and waits until it has spawned. Other tools then act as it.',
    inputSchema: CONNECT_SCHEMA,
    annotations: { openWorldHint: true },
    run: (args, { bots }) =>
      guarded(async () => {
        const handle = await bots.connect(args as ConnectOptions)
        return ok(`${handle.name} is connected; bots: ${bots.names().join(', ')}`, { bot: handle.name, bots: bots.names() })
      }),
  },
  {
    name: 'disconnect',
    description: 'Disconnects a bot (any running action is cancelled).',
    inputSchema: botSchema({}),
    run: ({ bot }, { bots }) =>
      guarded(async () => {
        const name = await bots.disconnect(bot as string | undefined)
        return ok(`${name} disconnected; bots: ${bots.names().join(', ') || 'none'}`, { bot: name, bots: bots.names() })
      }),
  },
  botTool({
    name: 'status',
    description:
      'The bot now: position, health, food, held item, players in view (with positions), entities nearby by kind, open forms and unread chat.',
    inputSchema: botSchema({}),
    annotations: { readOnlyHint: true },
    run: status,
  }),
  botTool({
    name: 'inventory',
    description: 'The non-empty stacks the bot carries: hotbar (slots 0-8), main inventory, off hand and armour.',
    inputSchema: botSchema({}),
    annotations: { readOnlyHint: true },
    run: (handle) => {
      const items = handle.bot.inventory.map((i) => ({ name: i.name, count: i.count, window: i.window, slot: i.slot }))
      const where = (i: (typeof items)[number]) => (i.window === 119 ? 'off hand' : i.window === 120 ? `armour ${i.slot}` : `slot ${i.slot}`)
      const text = items.map((i) => `${where(i)}: ${i.name ?? '?'} x${i.count}`).join('\n')
      return ok(text || 'empty', { items })
    },
  }),
  botTool({
    name: 'nearbyEntities',
    description: 'Tracked entities and players near the bot, nearest first, with the ids follow and attack take.',
    inputSchema: botSchema({
      maxDistance: { type: 'number', minimum: 1, description: 'Blocks from the bot; 32 when absent.' },
      kind: { type: 'string', description: 'Only this kind (`zombie` or `minecraft:zombie`).' },
    }),
    annotations: { readOnlyHint: true },
    run: (handle, args) => {
      const kind = typeof args.kind === 'string' ? kindName(args.kind) : undefined
      const entities = nearby(handle, (args.maxDistance as number | undefined) ?? 32)
        .filter((e) => !kind || e.kind === kind)
        .map((e) => ({ id: e.id, kind: e.kind, username: e.username, distance: e.distance, position: at(e.position) }))
      const text = entities.map((e) => `${e.id} ${e.username ?? e.kind} ${e.distance} away at ${fmtPos(e.position)}`).join('\n')
      return ok(text || 'nothing in range', { entities })
    },
  }),
  botTool({
    name: 'readChat',
    description: 'Chat lines the bot received since the last readChat (at most 200 kept), optionally waiting for one.',
    inputSchema: botSchema({
      wait: { type: 'number', minimum: 0, maximum: 60, description: 'Seconds to wait when nothing is unread; 0 when absent.' },
    }),
    annotations: { readOnlyHint: true },
    run: async (handle, args, signal) => {
      const lines = await handle.takeChat(((args.wait as number | undefined) ?? 0) * 1000, signal)
      return ok(lines.map(chatText).join('\n') || 'no new chat', {
        lines: lines.map((l) => ({ kind: l.kind, sender: l.sender, message: l.plain, ...(l.params.length > 0 && { params: l.params }) })),
      })
    },
  }),
  botTool({
    name: 'giveToPlayer',
    description: 'Walks to a player (within 2 blocks) and throws them items from the inventory.',
    inputSchema: botSchema(
      {
        player: { type: 'string', description: "The player's name." },
        item: { type: 'string', description: 'The item (`minecraft:bread`, or just `bread`).' },
        count: { type: 'integer', minimum: 1, description: 'How many; all the inventory has when absent.' },
      },
      ['player', 'item'],
    ),
    run: async (handle, args, signal) => {
      const { player, item, count } = args as { player: string; item: string; count?: number }
      await handle.action(signal, 120_000, (s) => handle.bot.goto(nearGoal(handle, player, 2), { signal: s }))
      const toss = (s: AbortSignal) => handle.bot.toss(item, count, { toward: eyesOf(handle, player), signal: s })
      const tossed = await handle.action(signal, 30_000, toss)
      return ok(`threw ${tossed} ${item} to ${player}${here(handle)}`, { tossed })
    },
  }),
  botTool({
    name: 'stop',
    description: "Cancels the bot's running actions (goto, follow, dig, ...) started through this server.",
    inputSchema: botSchema({}),
    run: (handle) => {
      const stopped = handle.stop()
      return ok(`stopped ${stopped} action${stopped === 1 ? '' : 's'}`, { stopped })
    },
  }),
]
