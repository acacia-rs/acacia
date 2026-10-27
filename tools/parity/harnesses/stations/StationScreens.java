package acacia.parity.gui;

import com.google.gson.JsonArray;
import com.google.gson.JsonNull;
import com.google.gson.JsonObject;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.screens.inventory.AbstractContainerScreen;
import net.minecraft.client.gui.screens.inventory.AnvilScreen;
import net.minecraft.client.gui.screens.inventory.BeaconScreen;
import net.minecraft.client.gui.screens.inventory.CartographyTableScreen;
import net.minecraft.client.gui.screens.inventory.EnchantmentScreen;
import net.minecraft.client.gui.screens.inventory.GrindstoneScreen;
import net.minecraft.client.gui.screens.inventory.SmithingScreen;
import net.minecraft.client.model.geom.EntityModelSet;
import net.minecraft.client.resources.model.ModelManager;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.ComponentSerialization;
import net.minecraft.resources.Identifier;
import net.minecraft.tags.EnchantmentTags;
import net.minecraft.tags.ItemTags;
import net.minecraft.world.inventory.AnvilMenu;
import net.minecraft.world.inventory.BeaconMenu;
import net.minecraft.world.inventory.CartographyTableMenu;
import net.minecraft.world.inventory.EnchantmentMenu;
import net.minecraft.world.inventory.GrindstoneMenu;
import net.minecraft.world.inventory.SmithingMenu;

/** The `stations` harness's screens by case kind, and the state each is given once open. */
final class StationScreens {
    private StationScreens() {}

    private static EntityModelSet models;

    static AbstractContainerScreen<?> create(ScreenKit kit, String kind) throws Exception {
        var inv = kit.inventory;
        return switch (kind) {
            case "anvil" -> new AnvilScreen(new AnvilMenu(0, inv), inv, StationOracle.title(kit, Component.translatable("container.repair")));
            case "grindstone" -> new GrindstoneScreen(new GrindstoneMenu(0, inv), inv, StationOracle.title(kit, Component.translatable("container.grindstone_title")));
            case "smithing" -> new SmithingScreen(new SmithingMenu(0, inv), inv, StationOracle.title(kit, Component.translatable("container.upgrade")));
            case "cartography" -> new CartographyTableScreen(new CartographyTableMenu(0, inv), inv,
                StationOracle.title(kit, Component.translatable("container.cartography_table")));
            case "beacon" -> new BeaconScreen(new BeaconMenu(0, inv), inv, StationOracle.title(kit, Component.translatable("container.beacon")));
            case "enchanting" -> {
                if (models == null) models = EntityModelSet.vanilla();
                var manager = Stub.alloc(ModelManager.class);
                Stub.put(manager, ModelManager.class, "entityModelSet", models);
                Stub.put(kit.mc, Minecraft.class, "modelManager", manager);
                yield new EnchantmentScreen(new EnchantmentMenu(0, inv), inv, StationOracle.title(kit, Component.translatable("container.enchant")));
            }
            default -> throw new IllegalArgumentException(kind);
        };
    }

    static void prepare(ScreenKit kit, AbstractContainerScreen<?> screen) throws Exception {
        switch (screen) {
            case EnchantmentScreen s -> enchanting(kit, s);
            case BeaconScreen s -> beacon(kit, s);
            case SmithingScreen s -> {
                for (int i = 0; i < kit.i("ticks", 0); i++) s.containerTick();
            }
            case CartographyTableScreen s -> kit.game.addProperty("extendable", s.getMenu().getSlot(0).getItem().is(ItemTags.EXTENDABLE_MAPS));
            default -> { }
        }
    }

    /** `clues` [[option, enchantment, level]]: the menu's clue data; the book's `open`/`flip` [now, before]. */
    static void enchanting(ScreenKit kit, EnchantmentScreen s) throws Exception {
        var menu = s.getMenu();
        var registry = kit.level.registryAccess().lookupOrThrow(Registries.ENCHANTMENT);
        var clues = new JsonArray();
        for (int i = 0; i < 3; i++) clues.add(JsonNull.INSTANCE);
        for (var e : kit.list("clues")) {
            var a = e.getAsJsonArray();
            int option = a.get(0).getAsInt();
            var holder = registry.get(net.minecraft.resources.ResourceKey.create(Registries.ENCHANTMENT, Identifier.parse(a.get(1).getAsString()))).orElseThrow();
            menu.setData(4 + option, registry.getId(holder.value()));
            menu.setData(7 + option, a.get(2).getAsInt());
            var clue = new JsonObject();
            clue.add("description", ComponentSerialization.CODEC.encodeStart(ScreenKit.ops(), holder.value().description()).getOrThrow());
            clue.addProperty("curse", holder.is(EnchantmentTags.CURSE));
            clue.addProperty("maxLevel", holder.value().getMaxLevel());
            clues.set(option, clue);
        }
        kit.game.add("clues", clues);
        var open = kit.c.getAsJsonArray("open");
        var flip = kit.c.getAsJsonArray("flip");
        s.open = open.get(0).getAsFloat();
        s.oOpen = open.get(1).getAsFloat();
        s.flip = flip.get(0).getAsFloat();
        s.oFlip = flip.get(1).getAsFloat();
    }

    /** `levels`, `primary`, `secondary` (effect ids or null) as the beacon's data, then a tick. */
    static void beacon(ScreenKit kit, BeaconScreen s) throws Exception {
        var menu = s.getMenu();
        menu.setData(0, kit.i("levels", 0));
        String[] keys = {"primary", "secondary"};
        for (int i = 0; i < 2; i++) {
            var id = kit.c.get(keys[i]);
            var effect = id == null || id.isJsonNull() ? null : BuiltInRegistries.MOB_EFFECT.get(Identifier.parse(id.getAsString())).orElseThrow();
            menu.setData(1 + i, BeaconMenu.encodeEffect(effect));
        }
        Stub.put(s, BeaconScreen.class, "primary", menu.getPrimaryEffect());
        Stub.put(s, BeaconScreen.class, "secondary", menu.getSecondaryEffect());
        s.containerTick();
    }
}
