"""Entity poses: each Java model the look draws, posed by the client's own setupAnim on a corpus of
render states, against our port (acacia-render entity/java/anim) drawn by the probe
acacia-render/examples/java_entities: each part's pose and each drawn corner. A layer whose
setupAnim we have not ported is one divergence per case. Case corpus after Horizon's
tools/test_entity_model_parity.py (MIT)."""
import itertools
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "entity_models"))
from harness import look  # noqa: E402

OWNER = "acacia-render entity (Java look entities)"

M = "net.minecraft.client.model."
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


def cases():
    out = []
    groups = [(QUADRUPEDS, QUADRUPED_EXTRAS), (HUMANOIDS, HUMANOID_EXTRAS)]
    for models, extras in groups:
        for name, model in models.items():
            for (pos, speed), (yaw, pitch), age, extra in itertools.product(WALKS, LOOKS, AGES, extras):
                state = {"walkAnimationPos": pos, "walkAnimationSpeed": speed, "yRot": yaw, "xRot": pitch, "ageInTicks": age,
                         "isBaby": name.endswith("_baby")}
                state.update({k: v for k, v in extra.items() if not k.startswith("@")})
                case = {"case": f"{name} #{len(out)}", "layer": f"minecraft:{name}#main", "model": model, "state": state}
                if "@swingHand" in extra:
                    case["swingHand"] = extra["@swingHand"]
                out.append(case)
    return out


def run(h):
    models = look(h)
    corpus = h.out / "cases.jsonl"
    corpus.write_text("".join(json.dumps(c) + "\n" for c in cases()))
    dump = h.oracle("acacia.parity.entity_poses.EntityPosesOracle", corpus, inputs=[corpus])
    h.read_probe(h.probe("acacia-render", "java_entities", "poses", models, dump / "poses.jsonl"))
