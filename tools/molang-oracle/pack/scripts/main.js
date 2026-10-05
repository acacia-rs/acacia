// Runs every case on its own probe entity and logs `ORACLE <index> <value>`; see ../../README.md.
import { world, system } from "@minecraft/server";
import { COUNT, PER_TYPE, STATEFUL } from "./cases.js";

const AT = { x: 0.5, y: 100, z: 0.5 };
const PARALLEL = 64;
const log = (text) => console.warn("ORACLE " + text);
const ticks = (n) => new Promise((resolve) => system.runTimeout(resolve, n));
const stateful = new Set(STATEFUL);

async function runCase(dimension, index) {
  let probe;
  try {
    probe = dimension.spawnEntity("oracle:probe" + Math.floor(index / PER_TYPE), AT);
    await ticks(2);
    if (stateful.has(index)) {
      probe.setProperty("oracle:step", index);
      // The property lands next tick, then the controller needs one to transition and one for a `truthy:` hit.
      await ticks(6);
    }
    probe.triggerEvent("oracle:e" + index);
    await ticks(2);
    log(index + " " + probe.getProperty("oracle:out"));
  } catch (error) {
    log(index + " error " + String(error).replace(/\s+/g, " "));
  }
  try {
    probe?.remove();
  } catch {}
}

async function runAll(dimension) {
  let next = 0;
  const worker = async () => {
    while (next < COUNT) await runCase(dimension, next++);
  };
  await Promise.all(Array.from({ length: PARALLEL }, worker));
  log("done");
}

world.afterEvents.worldLoad.subscribe(() => {
  const dimension = world.getDimension("overworld");
  dimension.runCommand("tickingarea add circle 0 100 0 2 oracle");
  let tries = 0;
  const id = system.runInterval(() => {
    // Spawning throws until the ticking area's chunks are loaded.
    try {
      dimension.spawnEntity("oracle:probe0", AT).remove();
    } catch (error) {
      if (++tries > 200) {
        system.clearRun(id);
        log("done spawn failed: " + error);
      }
      return;
    }
    system.clearRun(id);
    runAll(dimension);
  }, 5);
});
