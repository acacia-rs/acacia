import { AcaciaError, type Entity, type Goal } from '@acacia-rs/bot'

import type { BotHandle } from './bots.js'
import { kindName, type JsonSchema } from './tool.js'

export const ENTITY_PARAM: JsonSchema = {
  type: 'string',
  description: "An entity id (from nearbyEntities), a player's name, or an entity kind (`zombie`) for the nearest one.",
}

/** A player's eyes are this far above the feet an `Entity` position gives; other kinds get the same. */
const EYE_HEIGHT = 1.62

/** The entity `ref` names: a runtime id, a player's name, or the nearest entity of a kind. */
export function resolveEntity(handle: BotHandle, ref: string): Entity {
  const { bot } = handle
  if (/^\d+$/.test(ref)) {
    const byId = bot.entities[ref]
    if (byId) return byId
    throw new AcaciaError('NOT_POSSIBLE', `no entity ${ref} in view`)
  }
  const lower = ref.toLowerCase()
  const player = Object.values(bot.players).find((p) => p.username.toLowerCase() === lower)
  if (player?.entity) return player.entity
  if (player) throw new AcaciaError('NOT_POSSIBLE', `${player.username} is online but not in view`)
  const kind = kindName(lower)
  const nearest = bot.nearestEntity((e) => e.kind === kind)
  if (nearest) return nearest
  throw new AcaciaError('NOT_POSSIBLE', `no player or entity called ${ref} in view`)
}

/** A goal within `range` blocks of the cell of the entity `ref` names. */
export function nearGoal(handle: BotHandle, ref: string, range: number): Goal {
  const { x, y, z } = resolveEntity(handle, ref).position
  return { type: 'near', x: Math.floor(x), y: Math.floor(y), z: Math.floor(z), range }
}

/** Where the entity `ref` names has its eyes. */
export function eyesOf(handle: BotHandle, ref: string): { x: number; y: number; z: number } {
  const { x, y, z } = resolveEntity(handle, ref).position
  return { x, y: y + EYE_HEIGHT, z }
}
