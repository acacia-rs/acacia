//! Queued methods as bot actions.

mod containers;
mod crafting;
mod items;

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use acacia_api::{ApiError, ErrorCode, Face, FollowParams, Method, Reply};
use acacia_bot::pathfind::{Follow, Processes, Target};
use acacia_bot::{ActionError, Bot, CancelHandle};

use crate::convert;

type Action<'a> = Pin<Box<dyn Future<Output = Result<Reply, ApiError>> + 'a>>;

/// `bot.follow`'s range when the caller gives none.
const FOLLOW_RANGE: f32 = 2.0;

/// The action for a [`acacia_api::Lane::Queued`] method.
pub(crate) fn action(bot: &mut Bot, method: Method) -> Action<'_> {
    let done = |reply: Reply| move |r: Result<(), ActionError>| r.map(|()| reply).map_err(convert::action_error);
    match method {
        Method::Goto(p) => Box::pin(async move { done(Reply::Goto(()))(bot.goto(convert::goal(p.goal)).await) }),
        Method::Dig(p) => Box::pin(async move { done(Reply::Dig(()))(bot.break_block([p.x, p.y, p.z]).await) }),
        Method::AnswerForm(p) => {
            Box::pin(async move { done(Reply::AnswerForm(()))(bot.answer_form(p.form, convert::form_reply(p.answer)).await) })
        }
        Method::WaitForTicks(p) => Box::pin(async move { done(Reply::WaitForTicks(()))(bot.wait_ticks(p.ticks).await) }),
        Method::Follow(p) => Box::pin(async move { done(Reply::Follow(()))(follow(bot, p).await) }),
        Method::LookAt(p) => Box::pin(async move { done(Reply::LookAt(()))(bot.look_at([p.x, p.y, p.z]).await) }),
        Method::Attack(p) => Box::pin(async move { done(Reply::Attack(()))(bot.attack(p.entity.0).await) }),
        Method::Equip(p) => Box::pin(async move { done(Reply::Equip(()))(items::equip(bot, p).await) }),
        Method::PlaceBlock(p) => {
            let face = convert::face(p.face.unwrap_or(Face::Up));
            Box::pin(async move { done(Reply::PlaceBlock(()))(bot.place_block([p.x, p.y, p.z], face).await) })
        }
        Method::Craft(p) => Box::pin(async move { answer(crafting::craft(bot, p).await, Reply::Craft) }),
        Method::RecipesFor(p) => Box::pin(async move { Ok(Reply::RecipesFor(crafting::recipes_for(bot, &p.item))) }),
        Method::ContainerContents(p) => Box::pin(async move { answer(containers::contents(bot, p).await, Reply::ContainerContents) }),
        Method::Deposit(p) => Box::pin(async move { answer(containers::deposit(bot, p).await, Reply::Deposit) }),
        Method::Withdraw(p) => Box::pin(async move { answer(containers::withdraw(bot, p).await, Reply::Withdraw) }),
        Method::Smelt(p) => Box::pin(async move { answer(crafting::smelt(bot, p).await, Reply::Smelt) }),
        Method::Consume(p) => Box::pin(async move { done(Reply::Consume(()))(items::consume(bot, p).await) }),
        Method::Toss(p) => Box::pin(async move { answer(items::toss(bot, p).await, Reply::Toss) }),
        other => {
            let name = other.name();
            Box::pin(async move { Err(ApiError::new(ErrorCode::Internal, format!("{name} is not a queued method"))) })
        }
    }
}

fn answer<T>(r: Result<T, ActionError>, wrap: fn(T) -> Reply) -> Result<Reply, ApiError> {
    r.map(wrap).map_err(convert::action_error)
}

/// Runs acacia-bot's `Follow` process until cancelled; fails at once for an entity the bot does not track.
async fn follow(bot: &mut Bot, p: FollowParams) -> Result<(), ActionError> {
    if bot.state().entities.get(p.entity.0).is_none() {
        return Err(ActionError::NotPossible(format!("entity {} is not tracked", p.entity.0)));
    }
    let mut procs = Processes::new();
    procs.add(Follow::new(Target::Entity(p.entity.0), p.range.unwrap_or(FOLLOW_RANGE)), 0);
    bot.run(&mut procs).await
}

/// Re-fires a cancel on every poll once armed: a cancel only counts while the action waits (acacia_bot::cancel),
/// and one that came before the action's first wait would otherwise be lost.
pub(crate) struct Cancellable<'a> {
    action: Action<'a>,
    cancel: CancelHandle,
    armed: bool,
}

impl<'a> Cancellable<'a> {
    pub(crate) fn new(action: Action<'a>, cancel: CancelHandle) -> Self {
        Self { action, cancel, armed: false }
    }

    pub(crate) fn cancel(&mut self) {
        self.armed = true;
        self.cancel.cancel();
    }
}

impl Future for Cancellable<'_> {
    type Output = Result<Reply, ApiError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.armed {
            self.cancel.cancel();
        }
        self.action.as_mut().poll(cx)
    }
}
