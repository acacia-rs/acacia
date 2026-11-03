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
    "bot.snapshot" => Snapshot(BotParams) -> Snapshot, Immediate [];
    /// Changes the event groups the bot reports.
    "bot.setEvents" => SetEvents(SetEventsParams) -> (), Immediate [];
    /// Sends a chat line.
    "bot.chat" => Chat(ChatParams) -> (), Immediate [mcp];
    /// Runs a slash command (the leading `/` is optional).
    "bot.command" => Command(CommandParams) -> (), Immediate [mcp, privileged];
    /// Walks to a goal (physics bots only). Fails with `NOT_POSSIBLE` when there is no path.
    "bot.goto" => Goto(GotoParams) -> (), Queued [mcp];
    /// Breaks the block at a cell, as a player would (reach, tool, mining time).
    "bot.dig" => Dig(DigParams) -> (), Queued [mcp];
    /// Answers an open form after a human reading delay.
    "bot.answerForm" => AnswerForm(AnswerFormParams) -> (), Queued [mcp];
    /// Lets ticks pass (50 ms each) while the bot keeps playing.
    "bot.waitForTicks" => WaitForTicks(WaitForTicksParams) -> (), Queued [mcp];
    /// Keeps within `range` of a tracked entity, walking after it, until cancelled (physics and entity tracking).
    /// Never resolves on its own: it ends with `CANCELLED`, or `DISCONNECTED`.
    "bot.follow" => Follow(FollowParams) -> (), Queued [mcp];
    /// Turns the bot's eyes to a point.
    "bot.lookAt" => LookAt(LookAtParams) -> (), Queued [mcp];
    /// Swings at a tracked entity in reach. `NOT_POSSIBLE` out of reach or unknown, `NOT_VISIBLE` behind blocks.
    "bot.attack" => Attack(AttackParams) -> (), Queued [mcp];
    /// Holds or releases a movement key (physics bots). An action that moves the bot overrides the keys while it
    /// runs, and every queued call releases them when it ends.
    "bot.setControlState" => SetControlState(SetControlStateParams) -> (), Immediate [];
    /// Releases every movement key.
    "bot.clearControlStates" => ClearControlStates(BotParams) -> (), Immediate [];
    /// Moves an item from the main inventory to the hand (selected in the hotbar), an armour slot or the off hand.
    /// `NOT_POSSIBLE` when the inventory has none.
    "bot.equip" => Equip(EquipParams) -> (), Queued [mcp];
    /// Places the held block against a face of a block, waiting until the server shows it (`REJECTED` if it never
    /// does).
    "bot.placeBlock" => PlaceBlock(PlaceBlockParams) -> (), Queued [mcp];
    /// The nearest loaded blocks with any of the names, read from the bot's terrain without waiting for an action.
    "bot.findBlocks" => FindBlocks(FindBlocksParams) -> FindBlocksResult, Immediate [mcp];
}
