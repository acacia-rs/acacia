import { AcaciaError, type BlockPos, type ContainerItem, type ItemCount, type Recipe } from '@acacia/bot'

import type { Adapter } from './adapters.js'
import type { BotHandle } from './bots.js'
import { fmtPos, ok, withProps, type Args, type ToolResult } from './tool.js'

/** Blocks from the bot's feet within which a crafting table counts as in reach. */
const TABLE_REACH = 4

const nearestTable = (handle: BotHandle): BlockPos | undefined =>
  handle.bot.findBlocks({ matching: ['crafting_table'], maxDistance: TABLE_REACH, count: 1 })[0]

const CONTAINER_NOTE = 'Walk within reach of it first (goto with range 3).'

function moved(result: unknown, where: string, params: Args): ToolResult {
  const { moved } = result as { moved: number }
  return ok(`${moved} ${params.item} ${where}`, { moved })
}

const counted = (items: ItemCount[]) => items.map((i) => `${i.item} x${i.count}`).join(', ')

function recipeText(r: Recipe): string {
  const where = r.station !== 'crafting_table' ? r.station : r.fits2x2 ? '2x2 grid' : 'crafting table'
  return `${counted(r.outputs)} from ${counted(r.inputs)} (${where})`
}

/** Adapters of the inventory, crafting and container methods (see `Adapter`). */
export const ITEM_ADAPTERS: Partial<Record<string, Adapter>> = {
  'bot.craft': {
    schema: (base) =>
      withProps(base, {
        table: {
          type: 'boolean',
          description:
            'true: at the nearest crafting table within reach; false: in the bot\'s own 2x2 grid; absent: the table when one is in reach, else the grid.',
        },
      }),
    note: 'Walk within reach of a crafting table first for 3x3 recipes (findBlocks + goto).',
    params: ({ table, ...args }, handle) => {
      const pos = table === false ? undefined : nearestTable(handle)
      if (table === true && !pos) throw new AcaciaError('NOT_POSSIBLE', `no crafting table within ${TABLE_REACH} blocks`)
      return pos ? { ...args, table: pos } : args
    },
    timeout: () => 60_000,
    done: (result, params) => {
      const { crafted } = result as { crafted: number }
      const table = params.table as BlockPos | undefined
      return ok(`crafted ${crafted} ${params.item}${table ? ` at the crafting table at ${fmtPos(table)}` : ''}`, { crafted })
    },
  },
  'bot.containerContents': {
    note: CONTAINER_NOTE,
    done: (result) => {
      const { items } = result as { items: ContainerItem[] }
      return ok(items.map((i) => `slot ${i.slot}: ${i.item} x${i.count}`).join('\n') || 'empty', { items })
    },
  },
  'bot.deposit': {
    note: CONTAINER_NOTE,
    done: (result, params) => moved(result, `put into the container at ${fmtPos(params as BlockPos)}`, params),
  },
  'bot.withdraw': {
    note: CONTAINER_NOTE,
    done: (result, params) => moved(result, `taken from the container at ${fmtPos(params as BlockPos)}`, params),
  },
  'bot.recipesFor': {
    done: (result, params) => {
      const { recipes } = result as { recipes: Recipe[] }
      return ok(recipes.length ? recipes.map(recipeText).join('\n') : `no recipe makes ${params.item}`, { recipes })
    },
  },
}
