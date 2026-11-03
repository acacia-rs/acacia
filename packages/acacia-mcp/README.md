# @acacia/mcp

An [MCP](https://modelcontextprotocol.io) server that lets LLM agents (Claude Desktop, Claude Code, Mindcraft-style
setups) play Minecraft Bedrock through acacia bots. It runs on [`@acacia/bot`](../acacia-bot) in-process and
speaks MCP over stdio with the official TypeScript SDK (v2, `@modelcontextprotocol/server`). Pre-release: nothing
is published yet.

```sh
npx @acacia/mcp --host 127.0.0.1 --port 19132 --username Bot --offline
```

With `--host` and `--username` the bot connects at start (`--no-connect` waits for the `connect` tool, which also
adds more bots). `--transport auto|raknet|nethernet` picks the network (default auto); `--offline` is accepted for
familiarity, offline auth being the only kind so far. `--allow-commands` adds the `command` tool (slash commands);
it is off by default, and there is no raw packet tool.

## Setup

Claude Desktop (`claude_desktop_config.json`) or Claude Code (`.mcp.json`, or `claude mcp add`):

```json
{
  "mcpServers": {
    "minecraft": {
      "command": "npx",
      "args": ["@acacia/mcp", "--host", "127.0.0.1", "--port", "19132", "--username", "Bot", "--offline"]
    }
  }
}
```

From a checkout, use `"command": "node"` with `"args": ["<repo>/packages/acacia-mcp/dist/cli.js", ...]`, or
`claude mcp add minecraft -- node <repo>/packages/acacia-mcp/dist/cli.js --host 127.0.0.1 --username Bot`.

## Tools

Every tool but `connect` takes an optional `bot` (a username), needed only while several bots are connected.

| Tool | What it does |
|---|---|
| `connect` / `disconnect` | Connect a bot (defaults from the command line) / disconnect one |
| `status` | Position, health, food, held item, players in view with positions, entities nearby by kind, open forms, unread chat |
| `inventory` | The stacks the bot carries, by slot |
| `nearbyEntities` | Entities and players near the bot, nearest first, with their ids (`kind`, `maxDistance` filters) |
| `readChat` | Chat received since the last call; `wait` seconds for the next line |
| `stop` | Cancels the bot's running actions |
| `chat` | Sends a chat line |
| `goto` | Walks to `x, y, z`, a column `x, z`, or a `player`, within `range` |
| `follow` | Follows an entity or player for `seconds` (default 60) |
| `dig` | Breaks the block at a cell |
| `placeBlock` | Places the held block against a face of a block |
| `equip` | Moves an item to the hand, an armour slot or the off hand |
| `findBlocks` | The nearest loaded blocks with any of the names |
| `lookAt`, `attack`, `waitForTicks`, `answerForm` | Look at a point; hit an entity; let ticks pass; answer a server form |
| `command` | Runs a slash command (only with `--allow-commands`) |

`status` is the place to start: it is what an agent needs to pick its next action. Entity params (`follow`,
`attack`) take an id from `nearbyEntities`, a player's name, or a kind (`zombie`) for the nearest one. There is no
crafting tool yet; collecting (`findBlocks` + `dig`) and building (`equip` + `placeBlock`) are.

## Contract

- **Tools from the method table.** Every method of acacia-api's table (`crates/acacia-api/src/methods.rs`) flagged
  `mcp` is a tool named after it without `bot.`, with the table's doc comment as its description and its params
  schema (from schemars, in `methods.json`) as the input schema: `bot` made optional and schemars' numeric
  `format`s dropped. Methods also flagged `privileged` need `--allow-commands`. A method's adapter
  (`src/adapters.ts`) may reshape a tool to keep it flat (`goto`'s goal union, `answerForm`'s answer union), resolve
  names to entity ids, set its deadline and phrase its result. Describe a method by editing the Rust doc
  comments; `scripts/sync-methods.mjs` copies `methods.json` in on every build.
- **Results** are short text for the model plus `structuredContent` where there is data (`status`, `inventory`,
  `nearbyEntities`, `readChat`, `findBlocks`; `findBlocks` also declares its `outputSchema`). Actions that move the
  bot report where it ended up.
- **Errors** are tool results with `isError`, never protocol errors: the text is `CODE: message` with the acacia
  `ErrorCode` (`NOT_POSSIBLE`, `NOT_VISIBLE`, `REJECTED`, `TIMEOUT`, `CANCELLED`, `BAD_PARAMS`, `UNKNOWN_BOT`, ...),
  and `structuredContent.error` holds `{code, message}`. Calls with params that fail the schema are refused by the
  SDK the same way.
- **Long actions** wait until done. Cancelling the MCP request, or the `stop` tool, cancels the bot's call (within a
  tick). Each has a deadline after which it is cancelled with `TIMEOUT`: `goto` 120 s, `dig` 60 s, `waitForTicks`
  its ticks plus 10 s (at most 1200 ticks), other queued actions 30 s. `follow` never ends on its own, so its
  deadline is `seconds` and reaching it is success.
- **Bots** are kept by username (case-insensitive) for the life of the process; a tool without `bot` uses the only
  connected bot, connecting the command-line one first if none is. Each bot keeps its last 200 unread chat lines
  and the forms the server opened. The process exits when the client closes stdin, closing the bots first.

## Build and test

```sh
npm install
npm run build        # @acacia/bot's release build, then sync-methods + tsc
npm run build:test   # @acacia/bot's debug addon with TestServer, then sync-methods + tsc
npm test             # node:test: tool generation and calls over a fake transport; e2e.test.mjs over FakeServer
```

`e2e.test.mjs` runs the CLI on stdio under the SDK's client against FakeServer and prints the session; it skips when
the addon was built without `TestServer`.

Not yet: crafting and containers (no such methods), Streamable HTTP, MCP resources or notifications for events.
