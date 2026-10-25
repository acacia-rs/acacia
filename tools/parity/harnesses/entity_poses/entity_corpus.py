"""Render-state cases for the entity harnesses (acacia.parity.RenderStates reads them): each model
the look draws, by the class its renderer builds, over walks, looks, ages and per-family extras.
After Horizon's tools/test_entity_model_parity.py (MIT)."""
import itertools
import json

M ="net.minecraft.client.model."
QUADRUPEDS = {
    "cow": M + "animal.cow.CowModel", "cow_baby": M + "animal.cow.CowModel",
    "mooshroom": M + "animal.cow.CowModel", "mooshroom_baby": M + "animal.cow.BabyCowModel",
    "pig": M + "animal.pig.PigModel", "pig_baby": M + "animal.pig.BabyPigModel",
    "sheep": M + "animal.sheep.SheepModel", "sheep_baby": M + "animal.sheep.BabySheepModel",
    "chicken": M + "animal.chicken.AdultChickenModel", "chicken_baby": M + "animal.chicken.BabyChickenModel",
    "creeper": M + "monster.creeper.CreeperModel",
    "spider": M + "monster.spider.SpiderModel", "cave_spider": M + "monster.spider.SpiderModel",
}
HUMANOIDS = {
    "zombie": M + "monster.zombie.ZombieModel", "zombie_baby": M + "monster.zombie.BabyZombieModel",
    "husk": M + "monster.zombie.ZombieModel", "husk_baby": M + "monster.zombie.BabyZombieModel",
    "drowned": M + "monster.zombie.DrownedModel", "drowned_baby": M + "monster.zombie.BabyDrownedModel",
    "skeleton": M + "monster.skeleton.SkeletonModel", "stray": M + "monster.skeleton.SkeletonModel",
    "wither_skeleton": M + "monster.skeleton.SkeletonModel", "bogged": M + "monster.skeleton.BoggedModel",
}

WALKS = [(0.0, 0.0), (1.3, 0.4), (7.9, 1.0), (23.5, 0.75)]
LOOKS = [(0.0, 0.0), (35.0, -20.0), (-60.0, 40.0)]
AGES = [0.0, 37.25]
QUADRUPED_EXTRAS = [
    {},
    {"?headEatPositionScale": 1.0, "?headEatAngleScale": 0.83},
    {"?headEatPositionScale": 0.5, "?headEatAngleScale": 0.63, "ageScale": 0.5},
    {"?flap": 2.1, "?flapSpeed": 0.7},
]
HUMANOID_EXTRAS = [
    {},
    {"swingAnimation": 0.35, "@swingHand": "MAIN_HAND"},
    {"swingAnimation": 0.8, "@swingHand": "OFF_HAND"},
    {"isAggressive": True},
    {"isAggressive": True, "swingAnimation": 0.5, "@swingHand": "MAIN_HAND"},
    {"isAggressive": True, "rightHandItemStack": "minecraft:iron_sword"},
    {"isAggressive": True, "isBaby": True, "ageScale": 0.5, "rightHandItemStack": "minecraft:iron_sword", "swingAnimation": 0.2, "@swingHand": "MAIN_HAND"},
    {"isAggressive": True, "?isHoldingBow": True, "rightArmPose": "BOW_AND_ARROW", "rightHandItemStack": "minecraft:bow"},
    {"isAggressive": True, "rightArmPose": "THROW_TRIDENT", "rightHandItemStack": "minecraft:trident"},
    {"rightArmPose": "ITEM", "leftArmPose": "BLOCK"},
    {"rightArmPose": "CROSSBOW_CHARGE", "maxCrossbowChargeDuration": 25.0, "ticksUsingItem": 11.0, "isUsingItem": True},
    {"rightArmPose": "CROSSBOW_HOLD", "leftArmPose": "EMPTY"},
    {"leftArmPose": "SPYGLASS", "isUsingItem": True, "useItemHand": "OFF_HAND"},
    {"rightArmPose": "TOOT_HORN", "isUsingItem": True},
    {"rightArmPose": "BRUSH", "mainArm": "LEFT"},
    {"isCrouching": True},
    {"isPassenger": True},
    {"isFallFlying": True, "speedValue": 0.6},
    {"swimAmount": 1.0},
    {"swimAmount": 0.45, "swingAnimation": 0.3, "@swingHand": "MAIN_HAND"},
]


SHOWN = {"showHat": True, "showJacket": True, "showLeftPants": True, "showRightPants": True, "showLeftSleeve": True, "showRightSleeve": True}
PLAYERS = {"player": M + "player.PlayerModel", "player_slim": M + "player.PlayerModel"}
PLAYER_EXTRAS = [dict(SHOWN, **{("?" + k if k == "isAggressive" else k): v for k, v in extra.items()}) for extra in HUMANOID_EXTRAS] + [
    dict({k: False for k in SHOWN}, showHat=True, showLeftSleeve=True), {"isSpectator": True}]
CAPES = {"player#cape": M + "player.PlayerCapeModel"}
CAPE_EXTRAS = [
    {},
    {"capeFlap": 12.5, "capeLean": 40.0, "capeLean2": -7.5},
    {"capeFlap": -6.0, "capeLean": 150.0, "capeLean2": 20.0},
    {"capeFlap": 31.0, "capeLean": 3.25, "capeLean2": -20.0, "isCrouching": True},
    {"capeFlap": 4.0, "capeLean": 75.0, "capeLean2": 11.0, "swimAmount": 0.6},
]
WINGS = {"elytra": M + "object.equipment.ElytraModel", "elytra_baby": M + "object.equipment.ElytraModel"}
WING_EXTRAS = [
    {"elytraRotX": 0.2617994, "elytraRotY": 0.0, "elytraRotZ": -0.2617994},
    {"elytraRotX": 0.6981317, "elytraRotY": 0.08726646, "elytraRotZ": -0.7853982, "isCrouching": True},
    {"elytraRotX": 0.31, "elytraRotY": 0.02, "elytraRotZ": -1.3},
]

EVERY = [(QUADRUPEDS, QUADRUPED_EXTRAS), (HUMANOIDS, HUMANOID_EXTRAS), (PLAYERS, PLAYER_EXTRAS), (CAPES, CAPE_EXTRAS), (WINGS, WING_EXTRAS)]
ARMED = [(HUMANOIDS, HUMANOID_EXTRAS), (PLAYERS, PLAYER_EXTRAS)]


def cases(groups):
    out = []
    for models, extras in groups:
        for name, model in models.items():
            for (pos, speed), (yaw, pitch), age, extra in itertools.product(WALKS, LOOKS, AGES, extras):
                state = {"walkAnimationPos": pos, "walkAnimationSpeed": speed, "yRot": yaw, "xRot": pitch, "ageInTicks": age,
                         "isBaby": name.endswith("_baby")}
                state.update({k: v for k, v in extra.items() if not k.startswith("@")})
                layer = f"minecraft:{name}" if "#" in name else f"minecraft:{name}#main"
                case = {"case": f"{name} #{len(out)}", "layer": layer, "model": model, "state": state}
                case.update({k[1:]: v for k, v in extra.items() if k.startswith("@")})
                out.append(case)
    return out


def write(h, groups):
    """The cases of `groups` as JSON lines in the harness's run dir; returns the file."""
    corpus = h.out / "cases.jsonl"
    corpus.write_text("".join(json.dumps(c) + "\n" for c in cases(groups)))
    return corpus
