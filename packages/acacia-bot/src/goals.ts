import type { Goal } from './generated/types/Goal.js'

export type { Goal }

/** Feet in this cell. */
export const block = (x: number, y: number, z: number): Goal => ({ type: 'block', x, y, z })

/** Within `range` blocks (Euclidean) of the cell. */
export const near = (x: number, y: number, z: number, range: number): Goal => ({ type: 'near', x, y, z, range })

/** Any height in this column. */
export const xz = (x: number, z: number): Goal => ({ type: 'xz', x, z })

/** Feet at this height. */
export const y = (y: number): Goal => ({ type: 'y', y })

/** At least `range` blocks from the cell. */
export const away = (x: number, y: number, z: number, range: number): Goal => ({ type: 'away', x, y, z, range })

/** Whichever goal is reached first. */
export const any = (...goals: Goal[]): Goal => ({ type: 'any', goals })
