import type { BlockPos } from './generated/types/BlockPos.js'
import type { FormAnswer } from './generated/types/FormAnswer.js'
import type { Goal } from './generated/types/Goal.js'
import { BotReads } from './reads.js'
import type { CallOptions } from './session.js'

/**
 * One connected bot (contract: the package README). Queued actions (`goto`, `dig`, `answerForm`, `waitForTicks`)
 * run one at a time in call order; `chat`, `command` and the getters do not wait for them.
 */
export class Bot extends BotReads {
  /** Walks to `goal` (see `goals`); needs `physics`. Fails with `NOT_POSSIBLE` when there is no path. */
  goto(goal: Goal, options?: CallOptions): Promise<void> {
    return this.call('bot.goto', { goal }, options)
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
