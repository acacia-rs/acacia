package acacia.parity.gui;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.gui.screens.inventory.AbstractContainerScreen;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.ComponentSerialization;
import net.minecraft.world.inventory.AbstractContainerMenu;
import net.minecraft.world.item.ItemStack;

/**
 * The `trading` harness's oracle: the list screens (MerchantScreen, StonecutterScreen, LoomScreen)
 * over their real client-side menus, filled from the case, then one frame recorded by GuiCapture.
 * Every menu slot's stack is keyed by its index, the carried stack by -2; list entries' stacks by
 * the keys TradingScreens gives them. Stacks the screens copy before drawing (an offer's discounted
 * cost, a recipe's icon) are keyed by content: the first registered stack they match. The menu
 * state the server would send (the selected entry, the list the menu derived) is written to the
 * case's `menu` for the probe.
 */
public final class TradingOracle {
    public static final int CARRIED = -2;

    private record Content(ItemStack stack, int key) {}

    private final ScreenKit kit;
    private final List<Content> contents = new ArrayList<>();

    TradingOracle(ScreenKit kit) {
        this.kit = kit;
        kit.capture.keyOf = s -> {
            for (var c : contents) {
                if (ItemStack.matches(c.stack, s)) return c.key;
            }
            return -1;
        };
    }

    /** Keys a stack the screen draws as a copy: matched by content, first registered first. */
    void byContent(ItemStack stack, int key) {
        if (stack.isEmpty()) return;
        contents.add(new Content(stack, key));
        kit.items.add(Integer.toString(key), ScreenKit.resolved(stack));
    }

    Component title(Component fallback) {
        if (!kit.c.has("title") || kit.c.get("title").isJsonNull()) return fallback;
        return ComponentSerialization.CODEC.parse(ScreenKit.ops(), kit.c.get("title")).getOrThrow();
    }

    /** The case's slot contents (put straight into their containers) and carried stack. */
    void fill(AbstractContainerMenu menu) {
        for (var e : kit.list("slots")) {
            var a = e.getAsJsonArray();
            int index = a.get(0).getAsInt();
            if (index >= menu.slots.size()) continue;
            var slot = menu.slots.get(index);
            slot.container.setItem(slot.getContainerSlot(), ScreenKit.stack(a.get(1)));
        }
        menu.setCarried(kit.key(ScreenKit.stack(kit.c.get("carried")), CARRIED));
    }

    /** Keys every slot's stack by its index, as the menu holds them now. */
    void keySlots(AbstractContainerMenu menu) {
        for (int i = 0; i < menu.slots.size(); i++) kit.key(menu.slots.get(i).getItem(), i);
    }

    /** The case's mouse wheel steps, each a mouseScrolled away from any widget or slot. */
    void scroll(AbstractContainerScreen<?> screen) {
        for (var step : kit.list("scrolls")) screen.mouseScrolled(-1000, -1000, 0, step.getAsDouble());
    }

    JsonObject menuState() {
        var o = new JsonObject();
        kit.c.add("menu", o);
        return o;
    }

    static JsonArray strings(String... values) {
        var a = new JsonArray();
        for (var v : values) a.add(v);
        return a;
    }

    static void run(ScreenKit kit, JsonObject c) throws Exception {
        var oracle = new TradingOracle(kit);
        switch (c.get("kind").getAsString()) {
            case "merchant" -> TradingScreens.merchant(oracle, kit);
            case "stonecutter" -> TradingScreens.stonecutter(oracle, kit);
            case "loom" -> TradingScreens.loom(oracle, kit);
            default -> throw new IllegalArgumentException(c.get("kind").getAsString());
        }
    }

    public static void main(String[] args) throws Exception {
        ScreenKit.run(args, TradingOracle::run);
    }
}
