import { AcaciaError, type Form, type FormAnswer, type Goal } from '@acacia-rs/bot'

import type { BotHandle } from './bots.js'
import { ENTITY_PARAM, nearGoal, resolveEntity } from './entities.js'
import { ITEM_ADAPTERS } from './item-adapters.js'
import { badParams, distance, fmtPos, here, ok, props, withProps, type Adapter, type Args, type JsonSchema, type ToolResult } from './tool.js'

export type { Adapter }

function gotoGoal(args: Args, handle: BotHandle): Goal {
  const { x, y, z, player } = args as { x?: number; y?: number; z?: number; player?: string }
  const range = args.range as number | undefined
  if (player !== undefined) return nearGoal(handle, player, range ?? 2)
  if (x === undefined || z === undefined) throw badParams('goto needs x and z (and y), or player')
  if (y === undefined) return { type: 'xz', x, z }
  return range ? { type: 'near', x, y, z, range } : { type: 'block', x, y, z }
}

const INPUT_ELEMENTS = new Set(['input', 'toggle', 'slider', 'stepSlider', 'dropdown'])

function formAnswer(form: Form, args: Args): FormAnswer {
  if (args.close) return { type: 'close' }
  if (form.body.type === 'simple') {
    if (typeof args.button !== 'number') throw badParams(`form ${form.id} is a simple form: give button`)
    return { type: 'button', index: args.button }
  }
  if (form.body.type === 'modal') {
    if (typeof args.yes !== 'boolean') throw badParams(`form ${form.id} is a yes/no form: give yes`)
    return { type: 'modal', yes: args.yes }
  }
  const inputs = form.body.elements.filter((e) => INPUT_ELEMENTS.has(e.type))
  const values = (args.values ?? []) as unknown[]
  if (values.length !== inputs.length) throw badParams(`form ${form.id} has ${inputs.length} inputs: give one value each`)
  return {
    type: 'custom',
    values: inputs.map((element, i) => {
      const v = values[i]
      switch (element.type) {
        case 'input':
          return { type: 'text', value: String(v) }
        case 'toggle':
          return { type: 'toggle', value: Boolean(v) }
        case 'slider':
          return { type: 'slider', value: Number(v) }
        default: {
          const options = element.type === 'dropdown' ? element.options : element.type === 'stepSlider' ? element.steps : []
          const index = typeof v === 'number' ? v : options.indexOf(String(v))
          if (index < 0) throw badParams(`"${v}" is not one of ${options.join(', ')}`)
          return { type: 'choice', value: index }
        }
      }
    }),
  }
}

/** The forms the bot has open, latest last; `id` picks one. */
function openForm(handle: BotHandle, id: unknown): Form {
  const form = typeof id === 'number' ? handle.forms.get(id) : [...handle.forms.values()].at(-1)
  if (!form) throw new AcaciaError('NOT_POSSIBLE', id === undefined ? 'no form is open' : `no open form ${id}`)
  return form
}

const entityRef = (args: Args, handle: BotHandle): Args => ({ ...args, entity: resolveEntity(handle, String(args.entity)).id })

export const ADAPTERS: Partial<Record<string, Adapter>> = {
  'bot.goto': {
    schema: (base) => ({
      type: 'object',
      required: [],
      properties: {
        bot: props(base).bot,
        x: { type: 'integer' },
        y: { type: 'integer', description: 'Leave out to reach any height of the x, z column.' },
        z: { type: 'integer' },
        player: { type: 'string', description: "A player's name to walk to, instead of x, y, z." },
        range: { type: 'number', minimum: 0, description: 'Stop within this many blocks; 2 for a player, else 0 (the cell itself).' },
      },
    }),
    note: 'Give x, y, z (a cell), x, z (a column) or player.',
    params: (args, handle) => ({ goal: gotoGoal(args, handle) }),
    timeout: () => 120_000,
    done: (_, __, handle) => ok(`arrived${here(handle)}`),
  },
  'bot.follow': {
    schema: (base) =>
      withProps(base, {
        entity: ENTITY_PARAM,
        seconds: { type: 'number', minimum: 1, maximum: 600, description: 'How long to follow; 60 when absent.' },
      }),
    note: 'Here it ends after `seconds` (or on cancel / stop), reporting success.',
    params: (args, handle) => {
      const { seconds: _, ...rest } = entityRef(args, handle)
      return rest
    },
    timeout: (args) => ((args.seconds as number | undefined) ?? 60) * 1000,
    untilDeadline: (args, handle) => ok(`followed for ${args.seconds ?? 60} s${here(handle)}`),
  },
  'bot.attack': {
    schema: (base) => withProps(base, { entity: ENTITY_PARAM }),
    params: entityRef,
    timeout: () => 10_000,
  },
  'bot.answerForm': {
    schema: (base) => ({
      type: 'object',
      properties: {
        bot: props(base).bot,
        form: { type: 'integer', description: 'The form id (status lists open forms); the latest open form when absent.' },
        button: { type: 'integer', minimum: 0, description: 'Simple form: the button index, from 0.' },
        yes: { type: 'boolean', description: 'Yes/no (modal) form: the answer.' },
        values: {
          type: 'array',
          items: { type: ['string', 'number', 'boolean'] },
          description: 'Custom form: one value per input, in order (dropdowns take an option or its index).',
        },
        close: { type: 'boolean', description: 'Close the form instead of answering.' },
      },
      required: [],
    }),
    params: (args, handle) => {
      const form = openForm(handle, args.form)
      return { form: form.id, answer: formAnswer(form, args) }
    },
    done: (_, params, handle) => {
      handle.forms.delete(params.form as number)
      return ok(`answered form ${params.form}`)
    },
  },
  'bot.waitForTicks': {
    schema: (base) => withProps(base, { ticks: { ...props(base).ticks, maximum: 1200 } }),
    timeout: (args) => (args.ticks as number) * 50 + 10_000,
  },
  'bot.dig': { timeout: () => 60_000 },
  'bot.findBlocks': {
    done: (result, _, handle) => {
      const { blocks } = result as { blocks: { x: number; y: number; z: number }[] }
      const me = handle.bot.entity?.position
      const list = blocks.map((b) => `${b.x} ${b.y} ${b.z}${me ? ` (${distance(me, b)} away)` : ''}`)
      return ok(blocks.length === 0 ? 'none found' : `found ${blocks.length}: ${list.join('; ')}`, { blocks })
    },
  },
  ...ITEM_ADAPTERS,
}
