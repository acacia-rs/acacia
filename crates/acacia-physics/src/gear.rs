/// Active status effects that change movement. Speed/Slowness are not here: they are part of
/// the server's movement attribute (`PlayerState::movement_speed`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Effects {
    /// Amplifier (level - 1).
    pub jump_boost: Option<i32>,
    pub levitation: Option<i32>,
    pub slow_falling: bool,
    pub weaving: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Equipment {
    pub depth_strider: i32,
    pub soul_speed: i32,
    pub swift_sneak: i32,
    pub leather_boots: bool,
    pub elytra: bool,
    /// Glide wear the worn elytra takes before it stops working (431 − damage of 432); `None` when unknown.
    pub elytra_uses: Option<u32>,
}
