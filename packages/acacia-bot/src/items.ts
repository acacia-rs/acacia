import { AcaciaError } from './errors.js'
import type { BlockPos } from './generated/types/BlockPos.js'
import type { EquipDestination } from './generated/types/EquipDestination.js'
import type { Recipe } from './generated/types/Recipe.js'
import { BotReads, type Item } from './reads.js'
import type { CallOptions } from './session.js'

/** An item name (`stone` or `minecraft:stone`) or an `inventory` entry. */
export type ItemRef = string | Item

function nameOf(item: ItemRef): string {
  const name = typeof item === 'string' ? item : item.name
  if (name === null) throw new AcaciaError('BAD_PARAMS', 'the item has no name yet (no item table)')
  return name
}

/** A bot's inventory, crafting and container verbs (contract: the package README). */
export class BotItems extends BotReads {
  /** Moves an item to `destination` (default the hand). `NOT_POSSIBLE` when the inventory has none. */
  async equip(item: ItemRef, destination?: EquipDestination, options?: CallOptions): Promise<void> {
    return this.call('bot.equip', { item: nameOf(item), ...(destination === undefined ? {} : { destination }) }, options)
  }

  /**
   * Crafts at least `count` (default 1) of an item in the bot's own 2x2 grid, or at the crafting table at
   * `options.table` (within reach). Resolves with how many were made.
   */
  async craft(item: ItemRef, count = 1, options: CallOptions & { table?: BlockPos } = {}): Promise<number> {
    const { table, ...call } = options
    const params = { item: nameOf(item), count, ...(table === undefined ? {} : { table: { x: table.x, y: table.y, z: table.z } }) }
    return (await this.call('bot.craft', params, call)).crafted
  }

  /** The server's recipes that make an item. */
  async recipesFor(item: ItemRef, options?: CallOptions): Promise<Recipe[]> {
    return (await this.call('bot.recipesFor', { item: nameOf(item) }, options)).recipes
  }
}
