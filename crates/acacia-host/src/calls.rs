//! Queued methods as bot actions.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use acacia_api::{ApiError, ErrorCode, Method, Reply};
use acacia_bot::{Bot, CancelHandle};

use crate::convert;

type Action<'a> = Pin<Box<dyn Future<Output = Result<Reply, ApiError>> + 'a>>;

/// The action for a [`acacia_api::Lane::Queued`] method.
pub(crate) fn action(bot: &mut Bot, method: Method) -> Action<'_> {
    let done = |reply: Reply| move |r: Result<(), acacia_bot::ActionError>| r.map(|()| reply).map_err(convert::action_error);
    match method {
        Method::Goto(p) => Box::pin(async move { done(Reply::Goto(()))(bot.goto(convert::goal(p.goal)).await) }),
        Method::Dig(p) => Box::pin(async move { done(Reply::Dig(()))(bot.break_block([p.x, p.y, p.z]).await) }),
        Method::AnswerForm(p) => {
            Box::pin(async move { done(Reply::AnswerForm(()))(bot.answer_form(p.form, convert::form_reply(p.answer)).await) })
        }
        Method::WaitForTicks(p) => Box::pin(async move { done(Reply::WaitForTicks(()))(bot.wait_ticks(p.ticks).await) }),
        other => {
            let name = other.name();
            Box::pin(async move { Err(ApiError::new(ErrorCode::Internal, format!("{name} is not a queued method"))) })
        }
    }
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
