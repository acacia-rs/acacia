import { world, system, ItemStack, SignSide } from "@minecraft/server";

const log = (s) => console.warn(`ACTIONTEST ${s}`);
const say = (s) => {
  log(s);
  world.sendMessage(`ACTIONTEST ${s}`);
};
const TAG = "actiontest";

// Offsets from the player's feet block; the layout is drawn in README.md.
const STATIONS = {
  crafting_table: [-2, -2, "crafting_table"],
  furnace: [-1, -2, "furnace"],
  brewing_stand: [0, -2, "brewing_stand"],
  enchanting_table: [1, -2, "enchanting_table"],
  anvil: [2, -2, "anvil"],
  grindstone: [-2, 2, "grindstone"],
  stonecutter: [-1, 2, "stonecutter_block"],
  smithing_table: [0, 2, "smithing_table"],
  loom: [1, 2, "loom"],
  sign: [2, 2, 'standing_sign ["ground_sign_direction"=8]'],
  stone: [-2, 0, "stone"],
  bed: [2, 0, 'bed ["direction"=3,"head_piece_bit"=false]'],
  bed_head: [3, 0, 'bed ["direction"=3,"head_piece_bit"=true]'],
};
const POOL = [4, -3, 13, 3];

const ITEMS = [
  "oak_log 2", "raw_iron 2", "coal 2", "potion 3 0", "nether_wart 2", "blaze_powder 2", "lapis_lazuli 8",
  "iron_sword", "iron_pickaxe 1 200", "iron_ingot 4", "name_tag", "stone 16",
  "netherite_upgrade_smithing_template", "diamond_sword", "netherite_ingot", "banner 1 15", "red_dye 2",
  "writable_book", "bread 8", "iron_helmet", "iron_boots", "wooden_pickaxe", "diamond_pickaxe", "fishing_rod",
  "elytra", "firework_rocket 8", "emerald 32", "wheat 40", "iron_chestplate",
];

function build(player) {
  const dim = player.dimension;
  const [bx, by, bz] = [player.location.x, player.location.y, player.location.z].map(Math.floor);
  const run = (cmd) => {
    try {
      dim.runCommand(cmd);
    } catch (e) {
      log(`command failed: ${cmd}: ${e}`);
    }
  };
  for (const e of dim.getEntities({ tags: [TAG] })) e.remove();
  run(`fill ${bx - 9} ${by} ${bz - 5} ${bx + 14} ${by + 4} ${bz + 5} air`);
  run(`fill ${bx - 9} ${by - 1} ${bz - 5} ${bx + 14} ${by - 1} ${bz + 5} grass_block`);
  run(`fill ${bx + POOL[0]} ${by - 2} ${bz + POOL[1]} ${bx + POOL[2]} ${by - 1} ${bz + POOL[3]} water`);
  const scene = { base: [bx, by, bz] };
  for (const [name, [dx, dz, block]] of Object.entries(STATIONS)) {
    run(`setblock ${bx + dx} ${by} ${bz + dz} ${block}`);
    scene[name] = [bx + dx, by, bz + dz];
  }
  scene.water = [bx + 8, by - 1, bz];

  const spawn = (type, dx, dz, event) => {
    const [x, z] = [bx + dx + 0.5, bz + dz + 0.5];
    run(`summon ${type} ${x} ${by} ${z} 0 0 ${event ?? "minecraft:entity_spawned"}`);
    const [e] = dim.getEntities({ type, location: { x, y: by, z }, maxDistance: 1, excludeTags: [TAG] });
    if (!e) return log(`no ${type} spawned`);
    e.addTag(TAG);
    if (type !== "minecraft:boat") e.addEffect("slowness", 20000000, { amplifier: 255, showParticles: false });
    homes.set(e.id, { x, y: by, z });
  };
  homes.clear();
  spawn("minecraft:wandering_trader", -1, -1);
  spawn("minecraft:villager", -1, 1, "minecraft:spawn_farmer");
  spawn("minecraft:pig", 1, 1, "minecraft:on_saddled");
  spawn("minecraft:boat", 1, -1);
  for (const e of dim.getEntities({ type: "minecraft:item" })) e.remove();

  for (const rule of ["dodaylightcycle false", "domobspawning false", "doweathercycle false", "spawnradius 0"]) run(`gamerule ${rule}`);
  world.setTimeOfDay(18000);
  world.setDefaultSpawnLocation({ x: bx, y: by, z: bz });

  // BDS never sends the client script-made inventory changes (Container.addItem); commands sync.
  player.runCommand("clear @s");
  player.selectedSlotIndex = 0;
  player.runCommand("give @s golden_sword");
  player.runCommand("enchant @s sharpness 2");
  for (const item of ITEMS) player.runCommand(`give @s ${item}`);
  player.addLevels(30);
  player.addEffect("hunger", 60, { amplifier: 80, showParticles: false });

  const coords = Object.entries(scene).map(([k, v]) => `${k}=${v.join(",")}`).join(" ");
  // Hunger needs the effect to run out before the bot eats.
  system.runTimeout(() => say(`scene ${coords}`), 80);
  watchSign(dim, scene.sign);
}

// The trader's leashed llamas arrive ticks after it; the boat pulls them in and, both seats full,
// silently refuses the bot's mount.
world.afterEvents.entitySpawn.subscribe(({ entity }) => {
  if (entity.typeId === "minecraft:trader_llama") entity.remove();
});

// Mobs still get pushed around; put scene entities back unless someone rides them.
const homes = new Map();
system.runInterval(() => {
  for (const [id, home] of homes) {
    const e = world.getEntity(id);
    if (!e || e.getComponent("minecraft:rideable")?.getRiders().length) continue;
    const { x, z } = e.location;
    if (Math.hypot(x - home.x, z - home.z) <= 0.3) continue;
    log(`home ${e.typeId} from ${x.toFixed(3)},${z.toFixed(3)} at tick ${system.currentTick}`);
    e.teleport(home);
  }
}, 20);

// Sign edits have no stable event; poll the block entity.
let signWatch;
function watchSign(dim, [x, y, z]) {
  if (signWatch !== undefined) system.clearRun(signWatch);
  let last = "";
  signWatch = system.runInterval(() => {
    const sign = dim.getBlock({ x, y, z })?.getComponent("minecraft:sign");
    if (!sign) return;
    const text = JSON.stringify([sign.getText(SignSide.Front) ?? "", sign.getText(SignSide.Back) ?? ""]);
    if (text !== last) {
      last = text;
      say(`sign ${text}`);
    }
  }, 10);
}

world.afterEvents.playerSpawn.subscribe(({ player, initialSpawn }) => {
  if (!initialSpawn) return;
  system.runTimeout(() => {
    try {
      build(player);
    } catch (e) {
      log(`error ${e} ${e.stack}`);
    }
  }, 40);
});

// `/scriptevent actiontest:inv <item>`: the server's view of the sender's stacks of <item>.
// `/scriptevent actiontest:drop <item> <count>`: drops the stack at the sender's feet.
system.afterEvents.scriptEventReceive.subscribe(({ id, message, sourceEntity }) => {
  if (id !== "actiontest:drop" || !sourceEntity) return;
  const [item, count] = message.split(" ");
  sourceEntity.dimension.spawnItem(new ItemStack(item, Number(count ?? 1)), sourceEntity.location);
});

system.afterEvents.scriptEventReceive.subscribe(({ id, message, sourceEntity }) => {
  if (id === "actiontest:ent" && sourceEntity) return say(`ent ${JSON.stringify(entities(sourceEntity.dimension, message))}`);
  if (id !== "actiontest:inv" || !sourceEntity) return;
  const inv = sourceEntity.getComponent("minecraft:inventory").container;
  const found = [];
  for (let i = 0; i < inv.size; i++) {
    const s = inv.getItem(i);
    if (!s || (message && s.typeId !== message)) continue;
    const ench = s.getComponent("minecraft:enchantable")?.getEnchantments().map((e) => `${e.type.id}:${e.level}`) ?? [];
    found.push({ slot: i, id: s.typeId, count: s.amount, name: s.nameTag, damage: s.getComponent("minecraft:durability")?.damage, ench });
  }
  say(`inv ${JSON.stringify(found)}`);
});

// `/scriptevent actiontest:ent <type>`: where the scene's entities of <type> are and what they are doing.
function entities(dim, type) {
  return dim.getEntities({ type, tags: [TAG] }).map((e) => ({
    loc: [e.location.x, e.location.y, e.location.z].map((v) => Math.round(v * 100) / 100),
    variant: e.getComponent("minecraft:variant")?.value,
    mark: e.getComponent("minecraft:mark_variant")?.value,
    sleeping: e.isSleeping,
    families: e.getComponent("minecraft:type_family")?.getTypeFamilies(),
  }));
}

// Where the server puts a player leaving a vehicle: its position on the first ticks off the seat.
const offSeat = new Map(); // player id -> ticks since leaving a vehicle, -1 while riding
system.runInterval(() => {
  for (const p of world.getAllPlayers()) {
    if (p.getComponent("minecraft:riding")?.entityRidingOn) {
      offSeat.set(p.id, -1);
      continue;
    }
    const t = offSeat.get(p.id);
    if (t === undefined || t >= 2) continue;
    offSeat.set(p.id, t + 1);
    const { x, y, z } = p.location;
    say(`dismounted +${t + 1} ${x.toFixed(4)},${y.toFixed(4)},${z.toFixed(4)}`);
  }
}, 1);

// The server's side of an entity right-click: the before event fires only past the transaction checks.
const where = (e) => [e.location.x, e.location.y, e.location.z].map((v) => v.toFixed(3)).join(",");
world.beforeEvents.playerInteractWithEntity.subscribe(({ player, target, itemStack }) => {
  const riding = player.getComponent("minecraft:riding")?.entityRidingOn?.typeId;
  const v = target.getVelocity();
  const riders = target.getComponent("minecraft:rideable")?.getRiders().map((r) => r.typeId);
  log(`interact before ${target.typeId} at ${where(target)} v ${[v.x, v.y, v.z].map((c) => c.toFixed(4))} riders ${riders} valid ${target.isValid}; player ${where(player)} sneaking ${player.isSneaking} slot ${player.selectedSlotIndex} item ${itemStack?.typeId} riding ${riding}`);
});
world.afterEvents.playerInteractWithEntity.subscribe(({ target }) => log(`interact after ${target.typeId}`));

world.afterEvents.playerLeave.subscribe(({ playerName }) => log(`left ${playerName}`));
