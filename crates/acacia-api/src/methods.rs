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
    /// Crafts an item from the inventory through the recipe book, in the bot's own 2x2 grid or at a crafting table
    /// (`table`, within reach). `NOT_POSSIBLE` when no recipe fits the grid or ingredients are missing.
    "bot.craft" => Craft(CraftParams) -> CraftResult, Queued [mcp];
    /// The server's recipes that make an item, with what each uses up and where it is made.
    "bot.recipesFor" => RecipesFor(RecipesForParams) -> RecipesForResult, Queued [mcp];
    /// What a chest, barrel or other container holds: opens it (within reach), reads it and closes it.
    "bot.containerContents" => ContainerContents(ContainerParams) -> ContainerContents, Queued [mcp];
    /// Puts items from the inventory into a chest, barrel or other container (within reach). `NOT_POSSIBLE` when the
    /// inventory has none of the item or the container is full.
    "bot.deposit" => Deposit(TransferParams) -> TransferResult, Queued [mcp];
    /// Takes items out of a chest, barrel or other container (within reach) into the inventory. `NOT_POSSIBLE` when
    /// the container has none of the item or the inventory is full.
    "bot.withdraw" => Withdraw(TransferParams) -> TransferResult, Queued [mcp];
    /// Smelts items at a furnace, blast furnace or smoker (within reach): puts them and any fuel in, waits until they
    /// are done (10 s an item in a furnace, 5 s otherwise), takes the output and closes it.
    "bot.smelt" => Smelt(SmeltParams) -> SmeltResult, Queued [mcp];
    /// Eats or drinks: the held item, or `item` taken into the hand first. Resolves once the server has consumed it.
    /// `NOT_POSSIBLE` for something inedible, or food while not hungry.
    "bot.consume" => Consume(ConsumeParams) -> (), Queued [mcp];
    /// Throws items out of the inventory, a stack at a time. `NOT_POSSIBLE` when it has none.
    "bot.toss" => Toss(TossParams) -> TossResult, Queued [mcp];
}
