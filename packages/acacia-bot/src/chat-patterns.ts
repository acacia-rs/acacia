/** `bot.addChatPattern`'s options. */
export interface ChatPatternOptions {
  /** Keep matching after the first match (default true); false removes the pattern once it has matched. */
  repeat?: boolean
}

/** `bot.addChatPattern`'s patterns, matched against each chat line's `plain` text (contract: the package README). */
export class ChatPatterns {
  readonly #patterns = new Map<string, { regex: RegExp; repeat: boolean }>()

  add(name: string, regex: RegExp, { repeat = true }: ChatPatternOptions = {}): void {
    this.#patterns.set(name, { regex, repeat })
  }

  remove(name: string): boolean {
    return this.#patterns.delete(name)
  }

  /** The names of the patterns `text` matches, with their capture groups (`''` for one that took no part). */
  match(text: string): [name: string, groups: string[]][] {
    const matched: [string, string[]][] = []
    for (const [name, { regex, repeat }] of this.#patterns) {
      regex.lastIndex = 0
      const found = regex.exec(text)
      if (!found) continue
      matched.push([name, found.slice(1).map((g) => g ?? '')])
      if (!repeat) this.#patterns.delete(name)
    }
    return matched
  }
}
