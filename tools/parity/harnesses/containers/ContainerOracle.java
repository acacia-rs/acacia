package acacia.parity.gui;

import com.google.gson.JsonObject;
import java.util.Set;
import net.minecraft.client.gui.screens.inventory.AbstractContainerScreen;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.ComponentSerialization;
import net.minecraft.world.inventory.AbstractContainerMenu;
import net.minecraft.world.inventory.Slot;

/**
 * The `containers` harness's oracle: a real AbstractContainerScreen subclass over its real
 * client-side menu, the menu's slots and carried stack from the case (each keyed by its slot
 * index; the carried stack by -2), a quick-craft drag in progress, then one frame
 * (Screen.extractRenderStateWithTooltipAndSubtitles) recorded by GuiCapture. Screens: ContainerScreens.
 */
public final class ContainerOracle {
    public static final int CARRIED = -2;

    static Component title(ScreenKit kit, Component fallback) {
        if (!kit.c.has("title") || kit.c.get("title").isJsonNull()) return fallback;
        return ComponentSerialization.CODEC.parse(ScreenKit.ops(), kit.c.get("title")).getOrThrow();
    }

    /** The case's slot contents, carried stack and drag on `screen`'s menu. */
    static void fill(ScreenKit kit, AbstractContainerScreen<?> screen) throws Exception {
        AbstractContainerMenu menu = screen.getMenu();
        for (var e : kit.list("slots")) {
            var a = e.getAsJsonArray();
            int index = a.get(0).getAsInt();
            if (index >= menu.slots.size()) continue;
            var stack = kit.key(ScreenKit.stack(a.get(1)), index);
            Slot slot = menu.slots.get(index);
            slot.container.setItem(slot.getContainerSlot(), stack);
        }
        menu.setCarried(kit.key(ScreenKit.stack(kit.c.get("carried")), CARRIED));
        if (kit.c.has("quick")) {
            var q = kit.c.getAsJsonObject("quick");
            Stub.put(screen, AbstractContainerScreen.class, "isQuickCrafting", true);
            Stub.put(screen, AbstractContainerScreen.class, "quickCraftingType", q.get("type").getAsInt());
            @SuppressWarnings("unchecked")
            var slots = (Set<Slot>) Stub.get(screen, AbstractContainerScreen.class, "quickCraftSlots");
            for (var s : q.getAsJsonArray("slots")) {
                int index = s.getAsInt();
                if (index < menu.slots.size()) slots.add(menu.slots.get(index));
            }
            Stub.method(AbstractContainerScreen.class, "recalculateQuickCraftRemaining").invoke(screen);
        }
    }

    static void run(ScreenKit kit, JsonObject c) throws Exception {
        AbstractContainerScreen<?> screen = ContainerScreens.create(kit, c.get("kind").getAsString());
        kit.open(screen);
        fill(kit, screen);
        ContainerScreens.prepare(kit, screen);
        kit.frame(screen);
    }

    public static void main(String[] args) throws Exception {
        ScreenKit.run(args, ContainerOracle::run);
    }
}
