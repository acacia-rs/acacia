//! acacia-bot values to and from acacia-api's schema.

mod event;
mod snapshot;
mod world;

pub(crate) use event::event;
pub(crate) use snapshot::{hot, snapshot};
pub(crate) use world::block;

use acacia_api::{ApiError, ErrorCode, EventGroup, FormAnswer, FormValue, Goal};
use acacia_bot::forms::{FormReply, FormValue as BotFormValue};
use acacia_bot::pathfind::Goal as BotGoal;
use acacia_bot::{ActionError, ConnectError, Events};

pub(crate) fn action_error(e: ActionError) -> ApiError {
    let code = match &e {
        ActionError::Timeout => ErrorCode::Timeout,
        ActionError::Disconnected => ErrorCode::Disconnected,
        ActionError::Rejected(_) => ErrorCode::Rejected,
        ActionError::NotPossible(_) => ErrorCode::NotPossible,
        ActionError::NotVisible(_) => ErrorCode::NotVisible,
        ActionError::Cancelled => ErrorCode::Cancelled,
        _ => ErrorCode::Internal,
    };
    ApiError::new(code, e.to_string())
}

pub(crate) fn connect_error(e: ConnectError) -> ApiError {
    let code = match &e {
        ConnectError::Resolve => ErrorCode::Resolve,
        ConnectError::Io(_) => ErrorCode::Io,
        ConnectError::Disconnected(_) => ErrorCode::Disconnected,
        ConnectError::Timeout => ErrorCode::Timeout,
        ConnectError::NetherNet(_)
        | ConnectError::NetherNetNeedsOnline
        | ConnectError::NetherNetUnsupported
        | ConnectError::Signaling(_) => ErrorCode::NetherNet,
    };
    ApiError::new(code, e.to_string())
}

pub(crate) fn events(groups: &[EventGroup]) -> Events {
    groups.iter().fold(Events::NONE, |all, g| {
        all | match g {
            EventGroup::Chat => Events::CHAT,
            EventGroup::Forms => Events::FORMS,
            EventGroup::Health => Events::HEALTH,
            EventGroup::Players => Events::PLAYERS,
            EventGroup::Windows => Events::WINDOWS,
            EventGroup::Titles => Events::TITLES,
            EventGroup::Sleep => Events::SLEEP,
            EventGroup::Ticks => Events::TICKS,
        }
    })
}

pub(crate) fn goal(goal: Goal) -> BotGoal {
    match goal {
        Goal::Block { x, y, z } => BotGoal::Block([x, y, z]),
        Goal::Near { x, y, z, range } => BotGoal::Near([x, y, z], range),
        Goal::Xz { x, z } => BotGoal::XZ(x, z),
        Goal::Y { y } => BotGoal::Y(y),
        Goal::Away { x, y, z, range } => BotGoal::Away([x, y, z], range),
        Goal::Any { goals } => BotGoal::Or(goals.into_iter().map(self::goal).collect()),
    }
}

pub(crate) fn form_reply(answer: FormAnswer) -> FormReply {
    match answer {
        FormAnswer::Button { index } => FormReply::Button(index as usize),
        FormAnswer::Modal { yes } => FormReply::Modal(yes),
        FormAnswer::Custom { values } => FormReply::Custom(values.into_iter().map(form_value).collect()),
        FormAnswer::Close => FormReply::Close,
    }
}

fn form_value(value: FormValue) -> BotFormValue {
    match value {
        FormValue::Text(text) => BotFormValue::Text(text),
        FormValue::Toggle(on) => BotFormValue::Toggle(on),
        FormValue::Slider(v) => BotFormValue::Slider(v),
        FormValue::Choice(i) => BotFormValue::Choice(i as usize),
    }
}
