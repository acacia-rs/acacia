import type { BlockPos } from './generated/types/BlockPos.js'
import type { Control } from './generated/types/Control.js'
import type { Entity } from './generated/types/Entity.js'
import type { Face } from './generated/types/Face.js'
import type { FormAnswer } from './generated/types/FormAnswer.js'
import type { Goal } from './generated/types/Goal.js'
import type { Vec3 } from './generated/types/Vec3.js'
import type { CallOptions } from './session.js'
import { BotSurvival } from './survival.js'

/** An entity, or its `id`. */
export type EntityRef = Entity | string

const idOf = (entity: EntityRef): string => (typeof entity === 'string' ? entity : entity.id)

/**
 * One connected bot (contract: the package README). Queued actions (`goto`, `follow`, `dig`, ...) run one at a
 * time in call order; `chat`, `command`, the control states and the getters do not wait for them.
 */
export class Bot extends BotSurvival {
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

  /** Places the held block against `face` (default `up`) of the block at `referenceBlock`, once the server shows it. */
  placeBlock(referenceBlock: BlockPos, face?: Face, options?: CallOptions): Promise<void> {
    const { x, y, z } = referenceBlock
    return this.call('bot.placeBlock', { x, y, z, ...(face === undefined ? {} : { face }) }, options)
  }

  /** Breaks the block at a cell, as a player would; with `equipBestTool`, holding the fastest tool for it first. */
  dig(pos: BlockPos, options: CallOptions & { equipBestTool?: boolean } = {}): Promise<void> {
    const { equipBestTool, ...call } = options
    const params = { x: pos.x, y: pos.y, z: pos.z, ...(equipBestTool === undefined ? {} : { equipBestTool }) }
    return this.call('bot.dig', params, call)
  }

  answerForm(form: number | { id: number }, answer: FormAnswer, options?: CallOptions): Promise<void> {
    return this.call('bot.answerForm', { form: typeof form === 'number' ? form : form.id, answer }, options)
  }

  /** Right-clicks the block at `pos` with the held item (doors, buttons, levers, beds), on `face` (default `up`) when it shows. */
  activateBlock(pos: BlockPos, face?: Face, options?: CallOptions): Promise<void> {
    const { x, y, z } = pos
    return this.call('bot.activateBlock', { x, y, z, ...(face === undefined ? {} : { face }) }, options)
  }

  /** Uses the held item in the air (eat, throw, draw a bow, raise a shield); `deactivateItem` releases a bow or shield. */
  activateItem(options?: CallOptions): Promise<void> {
    return this.call('bot.activateItem', {}, options)
  }

  deactivateItem(options?: CallOptions): Promise<void> {
    return this.call('bot.deactivateItem', {}, options)
  }

  /** Right-clicks an entity in reach: trade, mount, feed, shear. */
  activateEntity(entity: EntityRef, options?: CallOptions): Promise<void> {
    return this.call('bot.activateEntity', { entity: idOf(entity) }, options)
  }

  swingArm(options?: CallOptions): Promise<void> {
    return this.call('bot.swingArm', {}, options)
  }

  /** Selects hotbar slot `slot` (0-8) as the held item. */
  setQuickBarSlot(slot: number, options?: CallOptions): Promise<void> {
    return this.call('bot.setQuickBarSlot', { slot }, options)
  }

  /** Presses "respawn" after dying (needed with `createBot({ autoRespawn: false })`). */
  respawn(options?: CallOptions): Promise<void> {
    return this.call('bot.respawn', {}, options)
  }

  /** Lets `ticks` ticks (50 ms each) pass while the bot keeps playing. */
  waitForTicks(ticks: number, options?: CallOptions): Promise<void> {
    return this.call('bot.waitForTicks', { ticks }, options)
  }
}
