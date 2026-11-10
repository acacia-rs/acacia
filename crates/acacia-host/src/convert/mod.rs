//! acacia-bot values to and from acacia-api's schema.

mod cursor;
mod event;
mod items;
mod snapshot;
mod world;

pub(crate) use cursor::block_at_cursor;
pub(crate) use event::event;
pub(crate) use items::{item, item_name, recipe, stack_name};
pub(crate) use snapshot::{cold, entities, hot, snapshot};
pub(crate) use world::{block, find_blocks, item_table};

use acacia_api::{
    ApiError, AutoEatPolicy, Control, EquipDestination, ErrorCode, EventGroup, Face, FoodPreference, FormAnswer, FormValue, Goal,
};
use acacia_bot::forms::{FormReply, FormValue as BotFormValue};
use acacia_bot::interact::Face as BotFace;
use acacia_bot::movement::Key;
use acacia_bot::survival::{AutoEat, Destination, FoodChoice};
use acacia_bot::pathfind::Goal as BotGoal;
use acacia_bot::{ActionError, ConnectError, Events};

pub(crate) fn face(face: Face) -> BotFace {
    match face {
        Face::Down => BotFace::Down,
        Face::Up => BotFace::Up,
        Face::North => BotFace::North,
        Face::South => BotFace::South,
        Face::West => BotFace::West,
        Face::East => BotFace::East,
    }
}

pub(crate) fn destination(to: EquipDestination) -> Destination {
    match to {
        EquipDestination::Hand => Destination::Hand,
        EquipDestination::Head => Destination::Head,
        EquipDestination::Torso => Destination::Chest,
        EquipDestination::Legs => Destination::Legs,
        EquipDestination::Feet => Destination::Feet,
        EquipDestination::OffHand => Destination::OffHand,
    }
}

pub(crate) fn key(control: Control) -> Key {
    match control {
        Control::Forward => Key::Forward,
        Control::Back => Key::Back,
        Control::Left => Key::Left,
        Control::Right => Key::Right,
        Control::Jump => Key::Jump,
        Control::Sneak => Key::Sneak,
        Control::Sprint => Key::Sprint,
    }
}

pub(crate) fn action_error(e: ActionError) -> ApiError {
    let code = match &e {
        ActionError::Timeout => ErrorCode::Timeout,
        ActionError::Disconnected => ErrorCode::Disconnected,
        ActionError::Rejected(_) => ErrorCode::Rejected,
        ActionError::NotPossible(_) | ActionError::Unsupported(_) => ErrorCode::NotPossible,
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
            EventGroup::Entities => Events::ENTITIES,
            EventGroup::EntityMoves => Events::ENTITY_MOVES,
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
        FormAnswer::Dialog { button, values } => FormReply::Dialog { button: button as usize, values: values.into_iter().map(form_value).collect() },
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

pub(crate) fn auto_eat(p: &AutoEatPolicy) -> AutoEat {
    let default = AutoEat::default();
    let prefer = p.prefer.map_or(default.prefer, |f| match f {
        FoodPreference::Saturation => FoodChoice::Saturation,
        FoodPreference::NoWaste => FoodChoice::NoWaste,
    });
    AutoEat { below: p.below.unwrap_or(default.below), prefer, from_inventory: p.from_inventory.unwrap_or(default.from_inventory) }
}
