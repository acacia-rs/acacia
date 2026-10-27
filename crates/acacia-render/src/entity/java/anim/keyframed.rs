//! The models Java animates with keyframes (26.3 client): bats, rabbits, frogs, baby axolotls,
//! armadillos, camels, sniffers, breezes, creakings, wardens, copper golems, nautiluses. An
//! `AnimationState` field of the render state holds the tick it started at ([`keyframes`]).

use std::f32::consts::PI;

use super::super::keyframes::{apply_state as play, apply_walk as walk, started};
use super::super::mth::{cos, pi_div, sin};
use super::super::poses::Poses;
use super::super::state::RenderState;
use super::head;

fn deg(d: f32) -> f32 {
    d * pi_div(180.0)
}

pub(super) fn bat(p: &mut Poses, s: &RenderState) {
    if s.flag("isResting") {
        p.y_rot("head", deg(s.y_rot()));
    }
    play(p, "BatAnimation.BAT_FLYING", s, "flyAnimationState", 1.0);
    play(p, "BatAnimation.BAT_RESTING", s, "restAnimationState", 1.0);
}

pub(super) fn rabbit(p: &mut Poses, s: &RenderState) {
    let class = if s.baby() { "BabyRabbitAnimation" } else { "RabbitAnimation" };
    if started(s, "idleHeadTiltAnimationState").is_none() {
        head(p, s);
    }
    play(p, &format!("{class}.HOP"), s, "hopAnimationState", 1.0);
    play(p, &format!("{class}.IDLE_HEAD_TILT"), s, "idleHeadTiltAnimationState", 1.0);
}

pub(super) fn frog(p: &mut Poses, s: &RenderState) {
    play(p, "FrogAnimation.FROG_JUMP", s, "jumpAnimationState", 1.0);
    play(p, "FrogAnimation.FROG_CROAK", s, "croakAnimationState", 1.0);
    play(p, "FrogAnimation.FROG_TONGUE", s, "tongueAnimationState", 1.0);
    if s.flag("isSwimming") {
        walk(p, "FrogAnimation.FROG_SWIM", s.walk_pos(), s.walk_speed(), 1.0, 2.5);
    } else {
        walk(p, "FrogAnimation.FROG_WALK", s.walk_pos(), s.walk_speed(), 1.5, 2.5);
    }
    play(p, "FrogAnimation.FROG_IDLE_WATER", s, "swimIdleAnimationState", 1.0);
    p.set_visible("croaking_body", started(s, "croakAnimationState").is_some());
}

pub(super) fn baby_axolotl(p: &mut Poses, s: &RenderState) {
    const A: &str = "BabyAxolotlAnimation.";
    if started(s, "walkAnimationState").is_some() {
        walk(p, &format!("{A}AXOLOTL_WALK_FLOOR"), s.walk_pos(), s.walk_speed(), 15.0, 30.0);
    }
    for (animation, field) in [
        ("BABY_AXOLOTL_SWIM", "swimAnimation"),
        ("WALK_FLOOR_UNDERWATER", "walkAnimationState"),
        ("BABY_AXOLOTL_IDLE_FLOOR", "idleOnGroundAnimationState"),
        ("IDLE_UNDERWATER", "idleUnderWaterAnimationState"),
        ("IDLE_FLOOR_UNDERWATER", "idleUnderWaterOnGroundAnimationState"),
        ("BABY_AXOLOTL_PLAY_DEAD", "playDeadAnimationState"),
    ] {
        play(p, &format!("{A}{animation}"), s, field, 1.0);
    }
}

pub(super) fn armadillo(p: &mut Poses, s: &RenderState) {
    let (class, prefix) = if s.baby() { ("BabyArmadilloAnimation", "ARMADILLO_BABY_") } else { ("ArmadilloAnimation", "ARMADILLO_") };
    let hiding = s.flag("isHidingInShell");
    p.set_skip_draw("body", hiding);
    for part in ["left_hind_leg", "right_hind_leg", "tail"] {
        p.set_visible(part, !hiding);
    }
    p.set_visible("cube", hiding);
    if !hiding {
        p.x_rot("head", deg(s.x_rot().clamp(-22.5, 25.0)));
        p.y_rot("head", deg(s.y_rot().clamp(-32.5, 32.5)));
        walk(p, &format!("{class}.{prefix}WALK"), s.walk_pos(), s.walk_speed(), 16.5, 2.5);
    }
    play(p, &format!("{class}.{prefix}ROLL_OUT"), s, "rollOutAnimationState", 1.0);
    play(p, &format!("{class}.{prefix}ROLL_UP"), s, "rollUpAnimationState", 1.0);
    play(p, &format!("{class}.{prefix}PEEK"), s, "peekAnimationState", 1.0);
}

pub(super) fn camel(p: &mut Poses, s: &RenderState) {
    let (class, prefix) = if s.baby() { ("CamelBabyAnimation", "CAMEL_BABY_") } else { ("CamelAnimation", "CAMEL_") };
    let y_rot = s.y_rot().clamp(-30.0, 30.0);
    let mut x_rot = s.x_rot().clamp(-25.0, 45.0);
    let cooldown = s.num("jumpCooldown");
    if cooldown > 0.0 {
        x_rot = (x_rot + 45.0 * cooldown / 55.0).clamp(-25.0, 70.0);
    }
    p.y_rot("head", deg(y_rot));
    p.x_rot("head", deg(x_rot));
    walk(p, &format!("{class}.{prefix}WALK"), s.walk_pos(), s.walk_speed(), 2.0, 2.5);
    for (animation, field) in [
        ("SIT", "sitAnimationState"),
        ("SIT_POSE", "sitPoseAnimationState"),
        ("STANDUP", "sitUpAnimationState"),
        ("IDLE", "idleAnimationState"),
        ("DASH", "dashAnimationState"),
    ] {
        play(p, &format!("{class}.{prefix}{animation}"), s, field, 1.0);
    }
}

pub(super) fn sniffer(p: &mut Poses, s: &RenderState) {
    const A: &str = "SnifferAnimation.SNIFFER_";
    head(p, s);
    let gait = if s.flag("isSearching") { "SNIFF_SEARCH" } else { "WALK" };
    walk(p, &format!("{A}{gait}"), s.walk_pos(), s.walk_speed(), 9.0, 100.0);
    for (animation, field) in [
        ("DIG", "diggingAnimationState"),
        ("LONGSNIFF", "sniffingAnimationState"),
        ("STAND_UP", "risingAnimationState"),
        ("HAPPY", "feelingHappyAnimationState"),
        ("SNIFFSNIFF", "scentingAnimationState"),
    ] {
        play(p, &format!("{A}{animation}"), s, field, 1.0);
    }
}

pub(super) fn breeze(p: &mut Poses, s: &RenderState) {
    for (animation, field) in [("IDLE", "idle"), ("SHOOT", "shoot"), ("SLIDE", "slide"), ("SLIDE_BACK", "slideBack"), ("INHALE", "inhale"), ("JUMP", "longJump")] {
        play(p, &format!("BreezeAnimation.{animation}"), s, field, 1.0);
    }
}

pub(super) fn creaking(p: &mut Poses, s: &RenderState) {
    const A: &str = "CreakingAnimation.CREAKING_";
    head(p, s);
    if s.flag("canMove") {
        walk(p, &format!("{A}WALK"), s.walk_pos(), s.walk_speed(), 1.0, 1.0);
    }
    play(p, &format!("{A}ATTACK"), s, "attackAnimationState", 1.0);
    play(p, &format!("{A}INVULNERABLE"), s, "invulnerabilityAnimationState", 1.0);
    play(p, &format!("{A}DEATH"), s, "deathAnimationState", 1.0);
}

pub(super) fn warden(p: &mut Poses, s: &RenderState) {
    head(p, s);
    let speed = (3.0 * s.walk_speed()).min(0.5);
    let pos = s.walk_pos() * 0.8662;
    let (c, sn) = (cos(pos), sin(pos));
    let capped = speed.min(0.35);
    p.get("head").z_rot += 0.3 * sn * speed;
    p.get("head").x_rot += 1.2 * cos(pos + pi_div(2.0)) * capped;
    p.z_rot("body", 0.1 * sn * speed);
    p.x_rot("body", 1.0 * c * capped);
    p.x_rot("left_leg", 1.0 * c * speed);
    p.x_rot("right_leg", 1.0 * cos(pos + PI) * speed);
    for (arm, x_rot, x) in [("left_arm", -(0.8 * c * speed), 13.0), ("right_arm", -(0.8 * sn * speed), -13.0)] {
        let part = p.get(arm);
        (part.x_rot, part.z_rot, part.y_rot) = (x_rot, 0.0, 0.0);
        (part.x, part.y, part.z) = (x, -13.0, 1.0);
    }
    let age = s.age() * 0.1;
    let (wc, ws) = (cos(age), sin(age));
    p.get("head").z_rot += 0.06 * wc;
    p.get("head").x_rot += 0.06 * ws;
    p.get("body").z_rot += 0.025 * ws;
    p.get("body").x_rot += 0.025 * wc;
    let tendril = s.num("tendrilAnimation") * ((s.age() as f64 * 2.25).cos() * std::f64::consts::PI * 0.1f32 as f64) as f32;
    p.x_rot("left_tendril", tendril);
    p.x_rot("right_tendril", -tendril);
    for (animation, field) in [
        ("ATTACK", "attackAnimationState"),
        ("SONIC_BOOM", "sonicBoomAnimationState"),
        ("DIG", "diggingAnimationState"),
        ("EMERGE", "emergeAnimationState"),
        ("ROAR", "roarAnimationState"),
        ("SNIFF", "sniffAnimationState"),
    ] {
        play(p, &format!("WardenAnimation.WARDEN_{animation}"), s, field, 1.0);
    }
}

pub(super) fn copper_golem(p: &mut Poses, s: &RenderState) {
    const A: &str = "CopperGolemAnimation.COPPER_GOLEM_";
    head(p, s);
    if !s.flag("rightHandItemState") && !s.flag("leftHandItemState") {
        walk(p, &format!("{A}WALK"), s.walk_pos(), s.walk_speed(), 2.0, 2.5);
    } else {
        walk(p, &format!("{A}WALK_ITEM"), s.walk_pos(), s.walk_speed(), 2.0, 2.5);
        let right = p.get("right_arm");
        (right.x_rot, right.y_rot, right.z_rot) = (right.x_rot.min(-0.872_664_63), right.y_rot.min(-0.113_446_4), right.z_rot.min(-0.064_577_185));
        let left = p.get("left_arm");
        (left.x_rot, left.y_rot, left.z_rot) = (left.x_rot.min(-0.872_664_63), left.y_rot.max(0.113_446_4), left.z_rot.max(0.064_577_185));
    }
    for (animation, field) in [
        ("IDLE", "idleAnimationState"),
        ("CHEST_INTERACTION_NOITEM_GET", "interactionGetItem"),
        ("CHEST_INTERACTION_NOITEM_NOGET", "interactionGetNoItem"),
        ("CHEST_INTERACTION_ITEM_DROP", "interactionDropItem"),
        ("CHEST_INTERACTION_ITEM_NODROP", "interactionDropNoItem"),
    ] {
        play(p, &format!("{A}{animation}"), s, field, 1.0);
    }
}

pub(super) fn nautilus(p: &mut Poses, s: &RenderState) {
    p.y_rot("body", deg(s.y_rot().clamp(-10.0, 10.0)));
    p.x_rot("body", deg(s.x_rot().clamp(-10.0, 10.0)));
    walk(p, "NautilusAnimation.SWIMMING", s.walk_pos() + s.age() / 5.0, s.walk_speed() + 0.2, 2.0, 3.0);
}
