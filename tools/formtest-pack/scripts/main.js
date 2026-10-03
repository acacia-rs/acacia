import { world, system } from "@minecraft/server";
import { ActionFormData, MessageFormData, ModalFormData } from "@minecraft/server-ui";

const log = (s) => console.warn(`FORMTEST ${s}`);

async function run(player) {
  const a = await new ActionFormData().title("Shop").body("Pick one").header("Top").button("Blocks").divider().button("Tools").show(player);
  log(`action canceled=${a.canceled} selection=${a.selection}`);
  const m = await new MessageFormData().title("Sure?").body("Really").button1("Yes").button2("No").show(player);
  log(`modal canceled=${m.canceled} selection=${m.selection}`);
  const c = await new ModalFormData()
    .title("Pay")
    .label("Send money")
    .textField("Player", "name")
    .toggle("Anonymous", { defaultValue: true })
    .slider("Amount", 1, 100, { valueStep: 1, defaultValue: 5 })
    .dropdown("Currency", ["coins", "gems"], { defaultValueIndex: 1 })
    .show(player);
  log(`custom canceled=${c.canceled} values=${JSON.stringify(c.formValues)}`);
  const x = await new ActionFormData().title("Close me").body("").button("x").show(player);
  log(`close canceled=${x.canceled} reason=${x.cancelationReason}`);
  world.sendMessage("§aFORMTEST done");
  player.onScreenDisplay.setActionBar("FORMTEST actionbar");
}

world.afterEvents.playerSpawn.subscribe(({ player, initialSpawn }) => {
  if (!initialSpawn) return;
  system.runTimeout(() => run(player).catch((e) => log(`error ${e}`)), 60);
});
