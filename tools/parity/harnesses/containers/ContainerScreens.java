package acacia.parity.gui;

import net.minecraft.client.gui.screens.inventory.AbstractContainerScreen;
import net.minecraft.client.gui.screens.inventory.BrewingStandScreen;
import net.minecraft.client.gui.screens.inventory.CrafterScreen;
import net.minecraft.world.inventory.BrewingStandMenu;
import net.minecraft.world.inventory.CrafterMenu;
import net.minecraft.client.gui.screens.inventory.ContainerScreen;
import net.minecraft.client.gui.screens.inventory.DispenserScreen;
import net.minecraft.client.gui.screens.inventory.HopperScreen;
import net.minecraft.client.gui.screens.inventory.ShulkerBoxScreen;
import net.minecraft.network.chat.Component;
import net.minecraft.world.inventory.ChestMenu;
import net.minecraft.world.inventory.DispenserMenu;
import net.minecraft.world.inventory.HopperMenu;
import net.minecraft.world.inventory.MenuType;
import net.minecraft.world.inventory.ShulkerBoxMenu;

/** The `containers` harness's screens by case kind, each over the menu the client opens for it. */
final class ContainerScreens {
    private ContainerScreens() {}

    static final MenuType<?>[] CHESTS = {MenuType.GENERIC_9x1, MenuType.GENERIC_9x2, MenuType.GENERIC_9x3, MenuType.GENERIC_9x4,
        MenuType.GENERIC_9x5, MenuType.GENERIC_9x6};

    static AbstractContainerScreen<?> create(ScreenKit kit, String kind) throws Exception {
        var inv = kit.inventory;
        return switch (kind) {
            case "chest" -> {
                int rows = kit.i("rows");
                var menu = new ChestMenu(CHESTS[rows - 1], 0, inv, new net.minecraft.world.SimpleContainer(9 * rows), rows);
                yield new ContainerScreen(menu, inv, ContainerOracle.title(kit, Component.translatable("container.chest")));
            }
            case "shulker" -> new ShulkerBoxScreen(new ShulkerBoxMenu(0, inv), inv, ContainerOracle.title(kit, Component.translatable("container.shulkerBox")));
            case "dispenser" -> new DispenserScreen(new DispenserMenu(0, inv), inv, ContainerOracle.title(kit, Component.translatable("container.dispenser")));
            case "hopper" -> new HopperScreen(new HopperMenu(0, inv), inv, ContainerOracle.title(kit, Component.translatable("container.hopper")));
            case "brewing" -> new BrewingStandScreen(new BrewingStandMenu(0, inv), inv, ContainerOracle.title(kit, Component.translatable("container.brewing")));
            case "crafter" -> new CrafterScreen(new CrafterMenu(0, inv), inv, ContainerOracle.title(kit, Component.translatable("container.crafter")));
            case "mount" -> mount(kit);
            default -> throw new IllegalArgumentException(kind);
        };
    }

    /**
     * ClientPacketListener.handleMountScreenOpen for the case's `entity` (tamed, baby, chested, named
     * as the case says) and `columns`; the probe gets the mount's figure and slot activity.
     */
    static AbstractContainerScreen<?> mount(ScreenKit kit) throws Exception {
        EntityStub.install(kit.mc);
        var type = net.minecraft.core.registries.BuiltInRegistries.ENTITY_TYPE.getValue(
            net.minecraft.resources.Identifier.parse(kit.c.get("entity").getAsString()));
        var entity = type.create(kit.level, net.minecraft.world.entity.EntitySpawnReason.LOAD);
        if (entity instanceof net.minecraft.world.entity.animal.equine.AbstractHorse horse) horse.setTamed(kit.bool("tamed", true));
        if (entity instanceof net.minecraft.world.entity.TamableAnimal tamable) tamable.setTame(kit.bool("tamed", true), false);
        if (entity instanceof net.minecraft.world.entity.animal.equine.AbstractChestedHorse chested) chested.setChest(kit.bool("chest", false));
        if (entity instanceof net.minecraft.world.entity.AgeableMob mob && kit.bool("baby", false)) mob.setAge(-24000);
        if (kit.c.has("name")) entity.setCustomName(net.minecraft.network.chat.Component.literal(kit.c.get("name").getAsString()));
        int columns = kit.i("columns", 0);
        var container = new net.minecraft.world.SimpleContainer(net.minecraft.world.inventory.AbstractMountInventoryMenu.getInventorySize(columns));
        AbstractContainerScreen<?> screen;
        if (entity instanceof net.minecraft.world.entity.animal.equine.AbstractHorse horse) {
            var menu = new net.minecraft.world.inventory.HorseInventoryMenu(0, kit.inventory, container, horse, columns);
            screen = new net.minecraft.client.gui.screens.inventory.HorseInventoryScreen(menu, kit.inventory, horse, columns);
        } else {
            var nautilus = (net.minecraft.world.entity.animal.nautilus.AbstractNautilus) entity;
            var menu = new net.minecraft.world.inventory.NautilusInventoryMenu(0, kit.inventory, container, nautilus, columns);
            screen = new net.minecraft.client.gui.screens.inventory.NautilusInventoryScreen(menu, kit.inventory, nautilus, columns);
        }
        var last = kit.list("lastMouse");
        if (last.size() == 2) {
            Stub.put(screen, net.minecraft.client.gui.screens.inventory.AbstractMountInventoryScreen.class, "xMouse", last.get(0).getAsFloat());
            Stub.put(screen, net.minecraft.client.gui.screens.inventory.AbstractMountInventoryScreen.class, "yMouse", last.get(1).getAsFloat());
        }
        kit.game.addProperty("mount", entity instanceof net.minecraft.world.entity.animal.equine.Llama ? "llama"
            : entity instanceof net.minecraft.world.entity.animal.equine.AbstractHorse ? "horse" : "nautilus");
        kit.game.add("figure", EntityStub.figure(entity));
        kit.game.addProperty("saddle", screen.getMenu().slots.get(0).isActive());
        kit.game.addProperty("armor", screen.getMenu().slots.get(1).isActive());
        kit.game.add("title", net.minecraft.network.chat.ComponentSerialization.CODEC.encodeStart(ScreenKit.ops(), screen.getTitle()).getOrThrow());
        return screen;
    }

    /** Per-screen state the case sets after the screen is open: the menu's data slots ({@code data}). */
    static void prepare(ScreenKit kit, AbstractContainerScreen<?> screen) throws Exception {
        int id = 0;
        for (var v : kit.list("data")) screen.getMenu().setData(id++, v.getAsInt());
    }
}
