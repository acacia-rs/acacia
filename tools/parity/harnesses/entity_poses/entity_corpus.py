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
    "zombie_villager": M + "monster.zombie.ZombieVillagerModel", "zombie_villager_baby": M + "monster.zombie.BabyZombieVillagerModel",
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

OTHERS = {
    "silverfish": M + "monster.silverfish.SilverfishModel", "endermite": M + "monster.endermite.EndermiteModel",
    "blaze": M + "monster.blaze.BlazeModel", "ghast": M + "monster.ghast.GhastModel",
    "iron_golem": M + "animal.golem.IronGolemModel", "snow_golem": M + "animal.golem.SnowGolemModel",
    "villager": M + "npc.VillagerModel", "villager_baby": M + "npc.BabyVillagerModel", "wandering_trader": M + "npc.VillagerModel",
    "witch": M + "monster.witch.WitchModel", "vex": M + "monster.vex.VexModel",
    "slime": M + "monster.slime.SlimeModel",
    "magma_cube": M + "monster.slime.MagmaCubeModel",
}
OTHER_EXTRAS = [
    {},
    {"?attackTicksRemaining": 6.5},
    {"?attackTicksRemaining": 0.0, "?offerFlowerTick": 33},
    {"?isUnhappy": True},
    {"?isCharging": True},
    {"?isCharging": True, "?rightHandItemState": True},
    {"?entityId": 17, "?isHoldingItem": True},
    {"?squish": 0.6},
    {"?squish": -0.4},
]
PIGLINS = {
    "piglin": M + "monster.piglin.AdultPiglinModel", "piglin_baby": M + "monster.piglin.BabyPiglinModel",
    "piglin_brute": M + "monster.piglin.AdultPiglinModel",
    "zombified_piglin": M + "monster.piglin.AdultZombifiedPiglinModel", "zombified_piglin_baby": M + "monster.piglin.BabyZombifiedPiglinModel",
    "enderman": M + "monster.enderman.EndermanModel",
}
PIGLIN_EXTRAS = HUMANOID_EXTRAS[:6] + [
    {"?armPose": "DANCING"},
    {"?armPose": "ATTACKING_WITH_MELEE_WEAPON", "rightHandItemStack": "minecraft:golden_sword"},
    {"?armPose": "CROSSBOW_HOLD"},
    {"?armPose": "CROSSBOW_CHARGE", "?maxCrossbowChageDuration": 25.0, "ticksUsingItem": 9.0},
    {"?armPose": "ADMIRING_ITEM", "mainArm": "LEFT"},
    {"?carriedBlock": True, "?isCreepy": True},
]
B = M + "animal."
ANIMALS = {
    "polar_bear": B + "polarbear.PolarBearModel", "polar_bear_baby": B + "polarbear.BabyPolarBearModel",
    "panda": B + "panda.PandaModel", "panda_baby": B + "panda.BabyPandaModel",
    "goat": B + "goat.GoatModel", "goat_baby": B + "goat.BabyGoatModel",
    "turtle": B + "turtle.AdultTurtleModel", "turtle_baby": B + "turtle.BabyTurtleModel",
    "hoglin": M + "monster.hoglin.HoglinModel", "hoglin_baby": M + "monster.hoglin.BabyHoglinModel",
    "zoglin": M + "monster.hoglin.HoglinModel", "zoglin_baby": M + "monster.hoglin.BabyHoglinModel",
    "wolf": B + "wolf.AdultWolfModel", "wolf_baby": B + "wolf.BabyWolfModel",
    "llama": B + "llama.LlamaModel", "llama_baby": B + "llama.BabyLlamaModel", "trader_llama": B + "llama.LlamaModel",
    "squid": B + "squid.SquidModel", "glow_squid_baby": B + "squid.SquidModel",
    "dolphin": B + "dolphin.DolphinModel", "dolphin_baby": B + "dolphin.DolphinModel",
}
ANIMAL_EXTRAS = [
    {},
    {"standScale": 0.7, "rammingXHeadRot": 0.6, "isOnLand": True, "isAngry": True, "isSitting": True, "isMoving": True,
     "hasChest": True, "tentacleAngle": 0.9, "attackAnimationRemainingTicks": 3, "isUnhappy": True},
    {"standScale": 1.0, "hasLeftHorn": False, "isOnLand": True, "isLayingEgg": True, "hasEgg": True, "isSitting": True,
     "shakeAnim": 0.9, "headRollAngle": 0.3, "tailAngle": 1.1, "attackAnimationRemainingTicks": 8,
     "sitAmount": 0.6, "isEating": True, "isSneezing": True, "sneezeTime": 9},
    {"shakeAnim": 1.3, "sitAmount": 1.0, "isScared": True, "lieOnBackAmount": 0.4, "rollAmount": 0.3, "isSneezing": True, "sneezeTime": 17},
]
FELINES = {
    "cat": B + "feline.AdultCatModel", "cat_baby": B + "feline.BabyCatModel",
    "ocelot": B + "feline.AdultOcelotModel", "ocelot_baby": B + "feline.BabyOcelotModel",
}
FELINE_EXTRAS = [
    {}, {"isCrouching": True}, {"isSprinting": True}, {"isSitting": True}, {"isCrouching": True, "isSitting": True},
    {"lieDownAmount": 0.7, "lieDownAmountTail": 0.4, "relaxStateOneAmount": 0.5},
    {"isSprinting": True, "lieDownAmount": 1.0, "lieDownAmountTail": 1.0},
]
F = B + "fish."
SWIMMERS = {
    "cod": F + "CodModel", "salmon": F + "SalmonModel", "salmon_large": F + "SalmonModel",
    "tropical_fish_large": F + "TropicalFishLargeModel", "tropical_fish_small": F + "TropicalFishSmallModel",
    "pufferfish_big": F + "PufferfishBigModel", "pufferfish_medium": F + "PufferfishMidModel", "pufferfish_small": F + "PufferfishSmallModel",
    "guardian": M + "monster.guardian.GuardianModel", "elder_guardian": M + "monster.guardian.GuardianModel",
    "phantom": M + "monster.phantom.PhantomModel", "allay": B + "allay.AllayModel",
}
SWIMMER_EXTRAS = [
    {},
    {"isInWater": True, "spikesAnimation": 0.4, "tailAnimation": 2.3, "flapTime": 13.5, "holdingAnimationProgress": 0.6},
    {"spikesAnimation": 1.0, "tailAnimation": -0.7, "flapTime": 41.0, "isDancing": True, "spinningProgress": 0.3, "isSpinning": True},
    {"isDancing": True, "spinningProgress": 0.6, "holdingAnimationProgress": 1.0},
]
E = B + "equine."
EQUINES = {
    "horse": E + "HorseModel", "horse_baby": E + "BabyHorseModel", "zombie_horse": E + "HorseModel",
    "skeleton_horse_baby": E + "BabyHorseModel", "donkey": E + "DonkeyModel", "donkey_baby": E + "BabyDonkeyModel",
    "mule": E + "DonkeyModel", "mule_baby": E + "BabyDonkeyModel",
}
EQUINE_EXTRAS = [
    {},
    {"eatAnimation": 0.6, "feedingAnimation": 0.5, "animateTail": True, "hasChest": True},
    {"standAnimation": 0.8, "eatAnimation": 0.2, "isInWater": True},
    {"standAnimation": 1.0, "animateTail": True, "feedingAnimation": 1.0},
]
CRITTERS = {
    "bee": B + "bee.AdultBeeModel", "bee_baby": B + "bee.BabyBeeModel", "parrot": B + "parrot.ParrotModel",
    "fox": B + "fox.AdultFoxModel",
}
CRITTER_EXTRAS = [
    {},
    {"isOnGround": True, "hasStinger": False, "pose": "STANDING", "flapAngle": 0.4, "isCrouching": True, "crouchAmount": 2.5},
    {"isAngry": True, "rollAmount": 0.5, "pose": "SITTING", "isSleeping": True, "headRollAngle": 0.2},
    {"rollAmount": 1.0, "pose": "PARTY", "flapAngle": 0.9, "isSitting": True, "isPouncing": True, "crouchAmount": 3.0},
    {"pose": "ON_SHOULDER", "flapAngle": 0.2, "isPouncing": True, "crouchAmount": 1.0, "headRollAngle": -0.3},
]
NETHER = {
    "strider": M + "monster.strider.AdultStriderModel", "strider_baby": M + "monster.strider.BabyStriderModel",
    "ravager": M + "monster.ravager.RavagerModel", "shulker": M + "monster.shulker.ShulkerModel",
    "wither": M + "monster.wither.WitherBossModel",
}
NETHER_EXTRAS = [
    {},
    {"isRidden": True, "attackTicksRemaining": 7.5, "peekAmount": 0.6, "yHeadRot": 30.0, "yBodyRot": -15.0, "bodyRot": 20.0},
    {"attackTicksRemaining": 3.0, "peekAmount": 1.0, "yHeadRot": -70.0},
    {"stunnedTicksRemaining": 13.0, "peekAmount": 0.2},
    {"roarAnimation": 4.5},
]
STILLS = {
    "axolotl": B + "axolotl.AdultAxolotlModel", "tadpole": B + "frog.TadpoleModel",
    "armor_stand": M + "object.armorstand.ArmorStandModel", "armor_stand_small": M + "object.armorstand.ArmorStandModel",
}
STILL_EXTRAS = [
    {},
    {"movingFactor": 1.0, "inWaterFactor": 1.0, "showArms": True},
    {"movingFactor": 0.4, "inWaterFactor": 0.3, "onGroundFactor": 0.7, "showBasePlate": False},
    {"movingFactor": 0.0, "inWaterFactor": 0.0, "onGroundFactor": 1.0, "playingDeadFactor": 0.6, "showArms": True},
    {"movingFactor": 1.0, "inWaterFactor": 0.0, "onGroundFactor": 1.0},
]
ILLAGERS = {name: M + "monster.illager.IllagerModel" for name in ["vindicator", "evoker", "pillager", "illusioner"]}
ILLAGER_EXTRAS = [
    {"?armPose": "CROSSED"},
    {"?armPose": "NEUTRAL", "isRiding": True},
    {"?armPose": "ATTACKING", "swingAnimation": 0.4},
    # IllagerRenderState.mainArm hides ArmedEntityRenderState's, which getMainHandItemState reads.
    {"?armPose": "ATTACKING", "rightHandItemState": True, "leftHandItemState": True, "swingAnimation": 0.4, "mainArm": "LEFT"},
    {"?armPose": "ATTACKING", "rightHandItemState": True, "swingAnimation": 0.7},
    {"?armPose": "SPELLCASTING"},
    {"?armPose": "BOW_AND_ARROW"},
    {"?armPose": "CROSSBOW_HOLD"},
    {"?armPose": "CROSSBOW_CHARGE", "maxCrossbowChargeDuration": 25.0, "ticksUsingItem": 9.0},
    {"?armPose": "CELEBRATING"},
]

EVERY = [(QUADRUPEDS, QUADRUPED_EXTRAS), (HUMANOIDS, HUMANOID_EXTRAS), (PLAYERS, PLAYER_EXTRAS), (CAPES, CAPE_EXTRAS), (WINGS, WING_EXTRAS), (OTHERS, OTHER_EXTRAS), (PIGLINS, PIGLIN_EXTRAS), (ILLAGERS, ILLAGER_EXTRAS), (ANIMALS, ANIMAL_EXTRAS), (FELINES, FELINE_EXTRAS), (SWIMMERS, SWIMMER_EXTRAS), (EQUINES, EQUINE_EXTRAS), (CRITTERS, CRITTER_EXTRAS), (NETHER, NETHER_EXTRAS), (STILLS, STILL_EXTRAS)]
ARMED = [(HUMANOIDS, HUMANOID_EXTRAS), (PLAYERS, PLAYER_EXTRAS)]


def cases(groups):
    out = []
    for models, extras in groups:
        for name, model in models.items():
            for (pos, speed), (yaw, pitch), age, extra in itertools.product(WALKS, LOOKS, AGES, extras):
                state = {"walkAnimationPos": pos, "walkAnimationSpeed": speed, "yRot": yaw, "xRot": pitch, "ageInTicks": age,
                         "isBaby": name.endswith("_baby")}
                state.update({k: v for k, v in extra.items() if not k.startswith("@")})
                # Not every state class has the living or humanoid fields.
                state = {k if k.startswith("?") else "?" + k: v for k, v in state.items()}
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
