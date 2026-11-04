import type { AutoEatPolicy } from './generated/types/AutoEatPolicy.js'
import type { BlockPos } from './generated/types/BlockPos.js'
import type { ItemCount } from './generated/types/ItemCount.js'
import { BotItems, cell } from './items.js'
import type { CallOptions } from './session.js'

/** A bot's tool, food, bed, sign and recipe-book verbs (contract: the package README). */
export class BotSurvival extends BotItems {
  /**
   * Holds the inventory's fastest tool for the block at `block`; resolves with the hotbar slot now held, or null
   * when nothing beats the bare hand (the held item is kept).
   */
  async equipBestTool(block: BlockPos, options?: CallOptions): Promise<number | null> {
    return (await this.call('bot.equipBestTool', cell(block), options)).slot
  }

  /** Turns auto-eat on (`true`, or a policy; `{}` and `true` take the defaults) or off. */
  setAutoEat(policy: boolean | AutoEatPolicy, options?: CallOptions): Promise<void> {
    const params = policy === false ? { enabled: false } : { enabled: true, ...(policy === true ? {} : policy) }
    return this.call('bot.setAutoEat', params, options)
  }

  /** Lies down in the bed at `bed`; resolves once the server shows the bot asleep (`REJECTED` with its reason). */
  sleep(bed: BlockPos, options?: CallOptions): Promise<void> {
    return this.call('bot.sleep', cell(bed), options)
  }

  wake(options?: CallOptions): Promise<void> {
    return this.call('bot.wake', {}, options)
  }

  /** Writes up to 4 lines (an array, or one string split at `\n`) on the front, or with `back` the back, of a sign. */
  writeSign(pos: BlockPos, lines: string | string[], options: CallOptions & { back?: boolean } = {}): Promise<void> {
    const { back, ...call } = options
    const text = typeof lines === 'string' ? lines.split('\n') : lines
    return this.call('bot.writeSign', { ...cell(pos), lines: text, ...(back === undefined ? {} : { back }) }, call)
  }

  /** The items the inventory can craft now, with what one craft makes; 3x3 recipes too with `table`. */
  async craftableItems(options: CallOptions & { table?: boolean } = {}): Promise<ItemCount[]> {
    const { table, ...call } = options
    return (await this.call('bot.craftableItems', table === undefined ? {} : { table }, call)).items
  }
}
