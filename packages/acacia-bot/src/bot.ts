import type { BlockPos } from './generated/types/BlockPos.js'
import type { Control } from './generated/types/Control.js'
import type { Entity } from './generated/types/Entity.js'
import type { FormAnswer } from './generated/types/FormAnswer.js'
import type { Goal } from './generated/types/Goal.js'
import type { Vec3 } from './generated/types/Vec3.js'
import { BotReads } from './reads.js'
import type { CallOptions } from './session.js'

/** An entity, or its `id`. */
export type EntityRef = Entity | string

const idOf = (entity: EntityRef): string => (typeof entity === 'string' ? entity : entity.id)

/**
 * One connected bot (contract: the package README). Queued actions (`goto`, `follow`, `dig`, ...) run one at a
 * time in call order; `chat`, `command`, the control states and the getters do not wait for them.
 */
export class Bot extends BotReads {
  /** Walks to `goal` (see `goals`); needs `physics`. Fails with `NOT_POSSIBLE` when there is no path. */
  goto(goal: Goal, options?: CallOptions): Promise<void> {
    return this.call('bot.goto', { goal }, options)
  }

  /**
   * Keeps within `range` blocks (default 2) of `entity` until `signal` aborts it, or the bot ends: it never
   * resolves on its own. Fails with `NOT_POSSIBLE` for an entity the bot does not track.
   */
  follow(entity: EntityRef, options: CallOptions & { range?: number } = {}): Promise<void> {
    const { range, ...call } = options
    return this.call('bot.follow', { entity: idOf(entity), ...(range === undefined ? {} : { range }) }, call)
  }

  /** Turns the bot's eyes to a point. */
  lookAt(pos: Vec3, options?: CallOptions): Promise<void> {
    return this.call('bot.lookAt', { x: pos.x, y: pos.y, z: pos.z }, options)
  }

  /** Swings at an entity in reach: `NOT_POSSIBLE` out of reach or untracked, `NOT_VISIBLE` behind blocks. */
  attack(entity: EntityRef, options?: CallOptions): Promise<void> {
    return this.call('bot.attack', { entity: idOf(entity) }, options)
  }

  /** Holds or releases a movement key; a running movement action overrides it, and each queued call's end releases it. */
  setControlState(control: Control, state: boolean, options?: CallOptions): Promise<void> {
    return this.call('bot.setControlState', { control, state }, options)
  }

  clearControlStates(options?: CallOptions): Promise<void> {
    return this.call('bot.clearControlStates', {}, options)
  }

  /** Breaks the block at a cell, as a player would. */
  dig(pos: BlockPos, options?: CallOptions): Promise<void> {
    return this.call('bot.dig', { x: pos.x, y: pos.y, z: pos.z }, options)
  }

  answerForm(form: number | { id: number }, answer: FormAnswer, options?: CallOptions): Promise<void> {
    return this.call('bot.answerForm', { form: typeof form === 'number' ? form : form.id, answer }, options)
  }

  /** Lets `ticks` ticks (50 ms each) pass while the bot keeps playing. */
  waitForTicks(ticks: number, options?: CallOptions): Promise<void> {
    return this.call('bot.waitForTicks', { ticks }, options)
  }
}
