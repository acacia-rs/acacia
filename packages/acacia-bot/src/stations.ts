import type { BeaconEffect } from './generated/types/BeaconEffect.js'
import type { BlockPos } from './generated/types/BlockPos.js'
import { cell, nameOf, slotOf, type ItemRef, type SlotRef } from './items.js'
import type { CallOptions } from './session.js'
import { BotSurvival } from './survival.js'

const some = <K extends string, V, W>(key: K, value: V | undefined, wire: (v: V) => W) =>
  (value === undefined ? {} : { [key]: wire(value) }) as Partial<Record<K, W>>
const same = <V>(v: V) => v

/**
 * A bot's workstation verbs (contract: the package README). Each opens the block at its first argument (within
 * reach), acts, takes the result into the inventory and closes it, also on failure. Items go by `SlotRef`.
 */
export class BotStations extends BotSurvival {
  /** Repairs, combines (`material`) and/or renames (`rename`) `input` at an anvil; needs one of the two. */
  anvil(anvil: BlockPos, input: SlotRef, options: CallOptions & { material?: SlotRef; rename?: string } = {}): Promise<void> {
    const { material, rename, ...call } = options
    const params = { ...cell(anvil), input: slotOf(input), ...some('material', material, slotOf), ...some('rename', rename, same) }
    return this.call('bot.anvil', params, call)
  }

  /** Enchants `item` with the table's offer `option` (0-2, cheapest first); needs its level and `option + 1` lapis. */
  enchant(table: BlockPos, item: SlotRef, option: number, options?: CallOptions): Promise<void> {
    return this.call('bot.enchant', { ...cell(table), item: slotOf(item), option }, options)
  }

  /** Disenchants `input` (curses stay), or merges it with `additional` of the same kind. */
  grindstone(grindstone: BlockPos, input: SlotRef, options: CallOptions & { additional?: SlotRef } = {}): Promise<void> {
    const { additional, ...call } = options
    return this.call('bot.grindstone', { ...cell(grindstone), input: slotOf(input), ...some('additional', additional, slotOf) }, call)
  }

  /** Upgrades or trims `base` with `addition` (ingot or trim material) and a smithing `template`. */
  smith(table: BlockPos, base: SlotRef, addition: SlotRef, template: SlotRef, options?: CallOptions): Promise<void> {
    const params = { ...cell(table), base: slotOf(base), addition: slotOf(addition), template: slotOf(template) }
    return this.call('bot.smith', params, options)
  }

  /** Cuts at least `count` (default 1) of an item; resolves with how many were made. */
  async stonecut(stonecutter: BlockPos, item: ItemRef, count = 1, options?: CallOptions): Promise<number> {
    return (await this.call('bot.stonecut', { ...cell(stonecutter), item: nameOf(item), count }, options)).made
  }

  /** Adds `pattern` (a Bedrock pattern id: `bo`, `cr`, ...) in `dye`'s colour to `banner`, with `patternItem` if needed. */
  loom(loom: BlockPos, banner: SlotRef, dye: SlotRef, pattern: string, options: CallOptions & { patternItem?: SlotRef } = {}): Promise<void> {
    const { patternItem, ...call } = options
    const params = { ...cell(loom), banner: slotOf(banner), dye: slotOf(dye), pattern, ...some('patternItem', patternItem, slotOf) }
    return this.call('bot.loom', params, call)
  }

  /**
   * Crafts `times` (default 1) with the filled maps in `map` and `additional` (an empty map clones, paper zooms out,
   * a glass pane locks), renamed to `rename` if given; resolves with the crafts made.
   */
  async cartography(
    table: BlockPos,
    map: SlotRef,
    options: CallOptions & { additional?: SlotRef; rename?: string; times?: number } = {},
  ): Promise<number> {
    const { additional, rename, times, ...call } = options
    const extra = { ...some('additional', additional, slotOf), ...some('rename', rename, same), ...some('times', times, same) }
    return (await this.call('bot.cartography', { ...cell(table), map: slotOf(map), ...extra }, call)).crafted
  }

  /** Sets a beacon to `primary` (plus `secondary` on a full pyramid), paying one item from `payment`. */
  beacon(beacon: BlockPos, primary: BeaconEffect, payment: SlotRef, options: CallOptions & { secondary?: BeaconEffect } = {}): Promise<void> {
    const { secondary, ...call } = options
    return this.call('bot.beacon', { ...cell(beacon), primary, payment: slotOf(payment), ...some('secondary', secondary, same) }, call)
  }

  /**
   * Brews one to three `bottles` with `ingredient`, adding a blaze powder when the stand has no fuel; resolves once the
   * bottles are back (about 20 s). A bottle named by item takes the next stack of it, so `['potion', 'potion']` is two.
   */
  brew(stand: BlockPos, bottles: SlotRef[], ingredient: ItemRef, options?: CallOptions): Promise<void> {
    return this.call('bot.brew', { ...cell(stand), bottles: bottles.map(slotOf), ingredient: nameOf(ingredient) }, options)
  }
}
