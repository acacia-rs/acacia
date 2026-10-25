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
            default -> throw new IllegalArgumentException(kind);
        };
    }

    /** Per-screen state the case sets after the screen is open: the menu's data slots ({@code data}). */
    static void prepare(ScreenKit kit, AbstractContainerScreen<?> screen) throws Exception {
        int id = 0;
        for (var v : kit.list("data")) screen.getMenu().setData(id++, v.getAsInt());
    }
}
