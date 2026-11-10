// Compiled by names.test.mjs (tsc --noEmit): the name unions accept vanilla and custom names and catch typos.
import type { BlockName, Bot, ItemName, VanillaBlockName, VanillaItemName } from '@acacia-rs/bot'

type Member<U, T extends U> = T

export type Stone = Member<VanillaBlockName, 'stone'>
export type Pickaxe = Member<VanillaItemName, 'diamond_pickaxe'>
// @ts-expect-error a typo is no vanilla name
export type Typo = Member<VanillaBlockName, 'stonee'>

export const short: BlockName = 'oak_log'
export const namespaced: BlockName = 'minecraft:oak_log'
export const custom: ItemName = 'my_pack:ruby'
// @ts-expect-error names are strings
export const numeric: ItemName = 42

export async function use(bot: Bot): Promise<void> {
  bot.findBlocks({ matching: ['diamond_ore', 'minecraft:deepslate_diamond_ore', 'my_pack:ruby_ore'] })
  await bot.equip('minecraft:diamond_pickaxe')
  await bot.craft('stick', 4)
  await bot.toss('dirt')
}
