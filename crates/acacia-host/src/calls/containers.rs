use acacia_api::{ContainerContents, ContainerItem, ContainerParams, TransferParams, TransferResult};
use acacia_bot::{ActionError, Bot};

use crate::convert;

/// Opens the container at `pos`, runs `act` on it and closes it, also when `act` failed.
async fn in_container<T>(bot: &mut Bot, pos: [i32; 3], act: impl AsyncFnOnce(&mut Bot) -> Result<T, ActionError>) -> Result<T, ActionError> {
    bot.open_container_at(pos).await?;
    let result = act(bot).await;
    let closed = bot.close_container().await;
    let value = result?;
    closed.map(|()| value)
}

pub(super) async fn contents(bot: &mut Bot, p: ContainerParams) -> Result<ContainerContents, ActionError> {
    in_container(bot, [p.x, p.y, p.z], async |bot: &mut Bot| {
        let items = &bot.state().items;
        let open = bot.open_container().ok_or_else(|| ActionError::NotPossible("the container closed at once".into()))?;
        let items = open.iter().map(|(slot, s)| ContainerItem { slot: slot as u32, item: convert::stack_name(items, s), count: u32::from(s.count) });
        Ok(ContainerContents { items: items.collect() })
    })
    .await
}

pub(super) async fn deposit(bot: &mut Bot, p: TransferParams) -> Result<TransferResult, ActionError> {
    let name = convert::item_name(&p.item);
    let moved = in_container(bot, [p.x, p.y, p.z], async |bot: &mut Bot| bot.deposit(&name, p.count.unwrap_or(u32::MAX)).await).await?;
    Ok(TransferResult { moved })
}

pub(super) async fn withdraw(bot: &mut Bot, p: TransferParams) -> Result<TransferResult, ActionError> {
    let name = convert::item_name(&p.item);
    let moved = in_container(bot, [p.x, p.y, p.z], async |bot: &mut Bot| bot.withdraw(&name, p.count.unwrap_or(u32::MAX)).await).await?;
    Ok(TransferResult { moved })
}
