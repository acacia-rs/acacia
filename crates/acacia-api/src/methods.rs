//! The method table: the one list of what a transport can call. Add a method here (its params in
//! [`crate::params`]) and the generated `methods.json` / `methods.ts` follow (`tests/generated.rs`).

use crate::params::*;
use crate::table::api;
use crate::Snapshot;

api! {
    /// Connects a bot and resolves once it has spawned. The `done` record names the new bot.
    "createBot" => CreateBot(CreateBotParams) -> CreateBotResult, Host [];
    /// Disconnects the bot (waiting up to 2 s for the server to free the session). The running call is
    /// cancelled and queued ones fail with `DISCONNECTED`; an `end` event comes before this resolves.
    "bot.close" => Close(BotParams) -> (), Immediate [];
    /// Cancels call `call`: removed if still queued, ended within a tick if running. Fails with `CANCELLED`.
    "cancel" => Cancel(CancelParams) -> (), Immediate [];
    /// The bot's state now, built on demand.
    "bot.snapshot" => Snapshot(BotParams) -> Snapshot, Immediate [mcp];
    /// Changes the event groups the bot reports.
    "bot.setEvents" => SetEvents(SetEventsParams) -> (), Immediate [];
    /// Sends a chat line.
    "bot.chat" => Chat(ChatParams) -> (), Immediate [mcp];
    /// Runs a slash command (the leading `/` is optional).
    "bot.command" => Command(CommandParams) -> (), Immediate [mcp];
    /// Walks to a goal (physics bots only). Fails with `NOT_POSSIBLE` when there is no path.
    "bot.goto" => Goto(GotoParams) -> (), Queued [mcp];
    /// Breaks the block at a cell, as a player would (reach, tool, mining time).
    "bot.dig" => Dig(DigParams) -> (), Queued [mcp];
    /// Answers an open form after a human reading delay.
    "bot.answerForm" => AnswerForm(AnswerFormParams) -> (), Queued [mcp];
    /// Lets ticks pass (50 ms each) while the bot keeps playing.
    "bot.waitForTicks" => WaitForTicks(WaitForTicksParams) -> (), Queued [mcp];
}
