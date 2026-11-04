//! The method table: the one list of what a transport can call. Add a method here (its params in
//! [`crate::params`]) and the generated `methods.json` / `methods.ts` follow (`tests/generated.rs`).

use crate::params::*;
use crate::table::api;
use crate::Snapshot;

api! {
    /// Connects a bot and resolves once it has spawned. The `done` record names the new bot.
    "createBot" => CreateBot(CreateBotParams) -> CreateBotResult, Host [];
    /// Signs a Microsoft account in without connecting, so later `createBot` calls find its tokens cached: an
    /// `msaCode` record asks a person to sign in when the cache has none. Fails `AUTH` (see the code).
    "signIn" => SignIn(SignInParams) -> SignInResult, Host [];
    /// Disconnects the bot (waiting up to 2 s for the server to free the session). The running call is
    /// cancelled and queued ones fail with `DISCONNECTED`; an `end` event comes before this resolves.
    "bot.close" => Close(BotParams) -> (), Immediate [];
    /// Cancels call `call`: removed if still queued, ended within a tick if running. Fails with `CANCELLED`.
    "cancel" => Cancel(CancelParams) -> (), Immediate [];
    /// Cancels a `createBot` or `signIn` call that has no bot yet (signing in, connecting or retrying its join): it
    /// fails with `CANCELLED`. A call that has already settled, or whose bot has spawned, is left alone.
    "cancelCall" => CancelCall(CancelCallParams) -> (), Host [];
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
    /// Right-clicks a block with the held item, facing it first: opens doors, gates and trapdoors, presses buttons
    /// and levers, uses beds, crafting tables and containers. Resolves once the click is sent, without waiting for
    /// the block to change. `NOT_POSSIBLE` out of reach, `NOT_VISIBLE` when blocks hide it.
    "bot.activateBlock" => ActivateBlock(ActivateBlockParams) -> (), Queued [mcp];
    /// Uses the held item in the air: eats, drinks, throws, draws a bow, raises a shield. Food finishes on its own;
    /// a bow, crossbow or shield stays in use until `bot.deactivateItem`.
    "bot.activateItem" => ActivateItem(BotParams) -> (), Queued [mcp];
    /// Releases the item in use: shoots a drawn bow, lowers a shield.
    "bot.deactivateItem" => DeactivateItem(BotParams) -> (), Queued [mcp];
    /// Right-clicks a tracked entity in reach, facing it first: trades with a villager, mounts, feeds, shears, names.
    /// `NOT_POSSIBLE` out of reach or untracked, `NOT_VISIBLE` behind blocks.
    "bot.activateEntity" => ActivateEntity(ActivateEntityParams) -> (), Queued [mcp];
    /// Swings the main hand, as a click on nothing does.
    "bot.swingArm" => SwingArm(BotParams) -> (), Queued [];
    /// Selects a hotbar slot (0-8) as the held item. `NOT_POSSIBLE` past 8.
    "bot.setQuickBarSlot" => SetQuickBarSlot(SetQuickBarSlotParams) -> (), Queued [mcp];
    /// Presses "respawn" on the death screen; a `respawn` event follows once the server has moved the bot. Bots
    /// respawn on their own unless created with `autoRespawn: false`. `NOT_POSSIBLE` while alive.
    "bot.respawn" => Respawn(BotParams) -> (), Queued [mcp];
    /// Holds the inventory's fastest tool for the block at a cell (main inventory, moved into the hotbar if needed),
    /// skipping tools about to break; keeps the held item when nothing beats the bare hand. Physics bots only:
    /// `NOT_POSSIBLE` for an unloaded cell.
    "bot.equipBestTool" => EquipBestTool(EquipBestToolParams) -> EquipBestToolResult, Queued [mcp];
    /// Turns auto-eat on or off: when hunger drops below `below` and the bot is idle (no queued call running, no
    /// window open) it eats from the hotbar, moving food up from the inventory if needed, and switches back.
    "bot.setAutoEat" => SetAutoEat(SetAutoEatParams) -> (), Queued [mcp];
    /// Lies down in a bed (within reach) and resolves once the server shows the bot asleep. `REJECTED` with the
    /// server's reason (daytime, monsters nearby, occupied), `NOT_POSSIBLE` for a block that is not a bed.
    "bot.sleep" => Sleep(SleepParams) -> (), Queued [mcp];
    /// Leaves the bed and resolves once the server shows the bot awake. `NOT_POSSIBLE` when not sleeping.
    "bot.wake" => Wake(BotParams) -> (), Queued [mcp];
    /// Writes a sign (within reach): clicks it unless its editor is open (as after placing one), types at a
    /// person's pace and closes the editor. `NOT_POSSIBLE` for a waxed sign, too much text, or when the server
    /// opened the other side.
    "bot.writeSign" => WriteSign(WriteSignParams) -> (), Queued [mcp];
    /// The items the inventory can craft now through the recipe book, in the bot's own 2x2 grid or (`table`) at a
    /// crafting table. Ingredients are matched greedily, so a recipe whose tags overlap can be missed.
    "bot.craftableItems" => CraftableItems(CraftableItemsParams) -> CraftableItemsResult, Queued [mcp];
    /// The block the bot is looking at: the first block shape its line of sight enters (collision or selection box,
    /// as the crosshair and anticheats see it), read from its terrain without waiting for an action.
    "bot.blockAtCursor" => BlockAtCursor(BlockAtCursorParams) -> BlockAtCursorResult, Immediate [mcp];
}
