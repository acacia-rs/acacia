import type { Goal } from './generated/types/Goal.js'

export type { Goal }

// Each class's instances are plain `Goal` objects on the wire (mineflayer-pathfinder's names; acacia's `Goal`).

/** Feet in this cell. */
export class GoalBlock {
  readonly type = 'block'
  constructor(readonly x: number, readonly y: number, readonly z: number) {}
}

/**
 * Within `range` blocks (Euclidean) of the cell.
 *
 * @example
 * ```js
 * await bot.goto(new GoalNear(10, 64, -3, 2))
 * ```
 */
export class GoalNear {
  readonly type = 'near'
  constructor(readonly x: number, readonly y: number, readonly z: number, readonly range: number) {}
}

/** Any height in this column. */
export class GoalXZ {
  readonly type = 'xz'
  constructor(readonly x: number, readonly z: number) {}
}

/** Feet at this height. */
export class GoalY {
  readonly type = 'y'
  constructor(readonly y: number) {}
}

/** At least `range` blocks from the cell: fleeing it. */
export class GoalAway {
  readonly type = 'away'
  constructor(readonly x: number, readonly y: number, readonly z: number, readonly range: number) {}
}

/**
 * Whichever goal is reached first.
 *
 * @example
 * ```js
 * await bot.goto(new GoalCompositeAny(goals.near(0, 64, 0, 1), goals.y(80)))
 * ```
 */
export class GoalCompositeAny {
  readonly type = 'any'
  readonly goals: Goal[]
  constructor(...goals: Goal[]) {
    this.goals = goals
  }
}

/** {@link GoalBlock}. */
export const block = (x: number, y: number, z: number): Goal => new GoalBlock(x, y, z)
/** {@link GoalNear}. */
export const near = (x: number, y: number, z: number, range: number): Goal => new GoalNear(x, y, z, range)
/** {@link GoalXZ}. */
export const xz = (x: number, z: number): Goal => new GoalXZ(x, z)
/** {@link GoalY}. */
export const y = (y: number): Goal => new GoalY(y)
/** {@link GoalAway}. */
export const away = (x: number, y: number, z: number, range: number): Goal => new GoalAway(x, y, z, range)
/** {@link GoalCompositeAny}. */
export const any = (...goals: Goal[]): Goal => new GoalCompositeAny(...goals)
