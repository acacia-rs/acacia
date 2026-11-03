import { AcaciaError } from './errors.js'
import type { BlockPos } from './generated/types/BlockPos.js'
import type { ContainerItem } from './generated/types/ContainerItem.js'
import type { EquipDestination } from './generated/types/EquipDestination.js'
import type { Recipe } from './generated/types/Recipe.js'
import type { Vec3 } from './generated/types/Vec3.js'
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
    const params = { item: nameOf(item), count, ...(table === undefined ? {} : { table: cell(table) }) }
    return (await this.call('bot.craft', params, call)).crafted
  }

  /** The server's recipes that make an item. */
  async recipesFor(item: ItemRef, options?: CallOptions): Promise<Recipe[]> {
    return (await this.call('bot.recipesFor', { item: nameOf(item) }, options)).recipes
  }

  /** What the container at `container` (within reach) holds: opened, read and closed in one call. */
  async containerContents(container: BlockPos, options?: CallOptions): Promise<ContainerItem[]> {
    return (await this.call('bot.containerContents', cell(container), options)).items
  }

  /** Puts up to `count` (default all) of an item into the container at `container`; resolves with how many moved. */
  async deposit(container: BlockPos, item: ItemRef, count?: number, options?: CallOptions): Promise<number> {
    return (await this.call('bot.deposit', transfer(container, item, count), options)).moved
  }

  /** Takes up to `count` (default all) of an item out of the container at `container`; resolves with how many moved. */
  async withdraw(container: BlockPos, item: ItemRef, count?: number, options?: CallOptions): Promise<number> {
    return (await this.call('bot.withdraw', transfer(container, item, count), options)).moved
  }

  /**
   * Smelts `count` (default 1) of an item at the furnace, blast furnace or smoker at `furnace`, adding `fuel`
   * (`fuelCount` of it, default 1) if given; resolves with how many items came out. Takes 10 s an item in a furnace.
   */
  async smelt(furnace: BlockPos, item: ItemRef, count = 1, options: CallOptions & { fuel?: ItemRef; fuelCount?: number } = {}): Promise<number> {
    const { fuel, fuelCount, ...call } = options
    const params = { ...cell(furnace), item: nameOf(item), count, ...(fuel === undefined ? {} : { fuel: nameOf(fuel), fuelCount: fuelCount ?? 1 }) }
    return (await this.call('bot.smelt', params, call)).smelted
  }

  /** Eats or drinks the held item, or `item` taken into the hand first; resolves once the server has consumed it. */
  async consume(item?: ItemRef, options?: CallOptions): Promise<void> {
    return this.call('bot.consume', item === undefined ? {} : { item: nameOf(item) }, options)
  }

  /** Throws up to `count` (default all) of an item, toward `options.toward` if given; resolves with how many went. */
  async toss(item: ItemRef, count?: number, options: CallOptions & { toward?: Vec3 } = {}): Promise<number> {
    const { toward, ...call } = options
    const params = { item: nameOf(item), ...(count === undefined ? {} : { count }), ...(toward === undefined ? {} : { toward: cell(toward) }) }
    return (await this.call('bot.toss', params, call)).tossed
  }
}

/** The point alone, without the extra fields an `Entity` position or block carries. */
const cell = ({ x, y, z }: BlockPos | Vec3) => ({ x, y, z })

const transfer = (container: BlockPos, item: ItemRef, count?: number) => ({
  ...cell(container),
  item: nameOf(item),
  ...(count === undefined ? {} : { count }),
})
