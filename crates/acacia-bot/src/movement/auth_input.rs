//! `PlayerAuthInput` for one simulated tick, laid out as the Android (touch) client sends it, the
//! platform online logins claim (docs/research/04-bedrock-gameplay-layer.md §1).

use acacia_client::proto::packets::{
    PlayerAuthInput, PlayerAuthInputInputMode, PlayerAuthInputInteractionModel, PlayerAuthInputPlayMode,
};
use acacia_client::proto::types::{InputData as F, Vec2f, Vec3f};
use acacia_physics::{Input, TickOutput};

use super::EYE_HEIGHT;

/// (start, stop) transitions this tick, plus the held states.
pub(super) struct Edges {
    pub sprint: (bool, bool),
    pub sneak: (bool, bool),
    pub jump: (bool, bool),
    pub swim: (bool, bool),
    pub glide: (bool, bool),
    pub sneaking: bool,
    pub sprinting: bool,
    /// The sprint key itself, which `Input::sprint` outlasts while a sprint continues.
    pub sprint_key: bool,
}

fn wrap_yaw(yaw: f32) -> f32 {
    let y = (yaw + 180.0).rem_euclid(360.0) - 180.0;
    if y.is_finite() { y } else { 0.0 }
}

fn vec3([x, y, z]: [f32; 3]) -> Vec3f {
    Vec3f { x, y, z }
}

/// Yaw wrapped to -180..180 and pitch clamped to -90..=90, as the client sends them.
pub(super) fn wire_rotation(yaw: f32, pitch: f32) -> (f32, f32) {
    (wrap_yaw(yaw), if pitch.is_finite() { pitch.clamp(-90.0, 90.0) } else { 0.0 })
}

/// `loaded`: past the loading screen, after which vanilla reports PlayMode Screen instead of Normal.
pub(super) fn build(input: &Input, out: &TickOutput, e: &Edges, tick: u64, loaded: bool) -> PlayerAuthInput {
    let (yaw, pitch) = wire_rotation(input.yaw, input.pitch);
    let [strafe, forward] = input.move_vector;

    let mut flags = Vec::with_capacity(12);
    let mut set = |on: bool, f: F| {
        if on {
            flags.push(f);
        }
    };
    set(forward > 0.0, F::Up);
    set(forward < 0.0, F::Down);
    set(strafe > 0.0, F::Left);
    set(strafe < 0.0, F::Right);
    set(forward > 0.0 && strafe > 0.0, F::UpLeft);
    set(forward > 0.0 && strafe < 0.0, F::UpRight);
    set(forward < 0.0 && strafe > 0.0, F::DownLeft);
    set(forward < 0.0 && strafe < 0.0, F::DownRight);
    set(input.jump, F::Jumping);
    set(input.jump, F::JumpDown);
    set(input.jump, F::JumpCurrentRaw);
    set(input.jump, F::WantUp);
    set(e.jump.0, F::JumpPressedRaw);
    set(e.jump.1, F::JumpReleasedRaw);
    set(out.jumped, F::StartJumping);
    set(e.sneaking, F::Sneaking);
    set(input.sneak, F::SneakDown);
    set(input.sneak, F::SneakCurrentRaw);
    set(input.want_down, F::WantDown);
    set(e.sneak.0, F::StartSneaking);
    set(e.sneak.0, F::SneakPressedRaw);
    set(e.sneak.1, F::StopSneaking);
    set(e.sneak.1, F::SneakReleasedRaw);
    // Mouse clients send the held key here; for a Touch player BDS reads it as the sprint state itself (it ends the
    // sprint when it drops and starts one, even sneaking, when it appears). The swim trigger needs it on a swim
    // start sent with a same-tick StopSprinting.
    set(e.sprinting || e.swim.0, F::Sprinting);
    set(e.sprint_key, F::SprintDown);
    set(e.sprint.0, F::StartSprinting);
    set(e.sprint.1, F::StopSprinting);
    set(e.swim.0, F::StartSwimming);
    set(e.swim.1, F::StopSwimming);
    // Landing ends a glide in the simulation on the tick after ground contact, where vanilla sends StopGliding.
    set(e.glide.0, F::StartGliding);
    set(e.glide.1, F::StopGliding);
    set(out.vertical_collision, F::VerticalCollision);
    set(out.horizontal_collision, F::HorizontalCollision);
    set(out.teleported, F::HandledTeleport);
    // Vanilla sets this on every tick (docs/research/vanilla-input-gaps.md).
    flags.push(F::BlockBreakingDelayEnabled);

    let (ys, yc) = yaw.to_radians().sin_cos();
    let (ps, pc) = pitch.to_radians().sin_cos();
    PlayerAuthInput {
        pitch,
        yaw,
        // A fixed offset whatever the pose: the servers read feet as position - 1.62 even when sneaking.
        position: vec3([out.position[0], out.position[1] + EYE_HEIGHT, out.position[2]]),
        move_vector: Vec2f { x: out.move_vector[0], z: out.move_vector[1] },
        head_yaw: yaw,
        input_data: flags,
        input_mode: PlayerAuthInputInputMode::Touch,
        play_mode: if loaded { PlayerAuthInputPlayMode::Screen } else { PlayerAuthInputPlayMode::Normal },
        interaction_model: PlayerAuthInputInteractionModel::Touch,
        interact_rotation: Vec2f { x: pitch, z: yaw },
        tick,
        delta: vec3(out.delta),
        transaction: None,
        item_stack_request: None,
        block_action: None,
        vehicle_rotation: None,
        predicted_vehicle: None,
        analogue_move_vector: Vec2f { x: 0.0, z: 0.0 },
        camera_orientation: Vec3f { x: -ys * pc, y: -ps, z: yc * pc },
        raw_move_vector: Vec2f { x: strafe, z: forward },
    }
}

#[cfg(test)]
mod tests {
    use super::wrap_yaw;

    #[test]
    fn yaw_wraps_into_range() {
        assert_eq!(wrap_yaw(190.0), -170.0);
        assert_eq!(wrap_yaw(-190.0), 170.0);
        assert_eq!(wrap_yaw(45.0), 45.0);
        assert_eq!(wrap_yaw(f32::NAN), 0.0);
    }
}
