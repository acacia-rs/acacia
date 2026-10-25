package acacia.parity.gui;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import java.util.Set;
import net.minecraft.client.gui.components.EditBox;
import net.minecraft.client.gui.screens.inventory.AbstractContainerScreen;
import net.minecraft.client.gui.screens.inventory.CreativeModeInventoryScreen;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.network.chat.ComponentSerialization;
import net.minecraft.resources.Identifier;
import net.minecraft.tags.TagKey;
import net.minecraft.util.Util;
import net.minecraft.world.flag.FeatureFlags;
import net.minecraft.world.inventory.Slot;
import net.minecraft.world.item.CreativeModeTab;
import net.minecraft.world.item.CreativeModeTabs;
import net.minecraft.world.item.Item;

/**
 * The `creative` harness's oracle: CreativeModeInventoryScreen over its real ItemPickerMenu and the
 * creative tabs CreativeModeTabs builds from the vanilla registries, with the case's selected tab,
 * operator tab, inventory, carried stack, search box, scroll and player figure; one frame recorded
 * by GuiCapture. Stubs: CreativeStubs.
 *
 * <p>Draw keys: a menu slot's stack is keyed by its position in menu.slots, a tab's icon by
 * 1000 + its index in CreativeModeTabs.tabs(), the carried stack by -2. The probe's `game`: the
 * tabs (id, type, row, column, alignedRight, showTitle, canScroll, background, title), the menu's
 * item count and empty entries, and for the hovered stack (resolved under its key) the tabs that
 * contain it, its tags and the screen's visible tags. The search box's results are the game's.
 */
public final class CreativeOracle {
    public static final int ICONS = 1000;

    static void search(ScreenKit kit, CreativeModeInventoryScreen screen) throws Exception {
        var selected = (CreativeModeTab) Stub.get(null, CreativeModeInventoryScreen.class, "selectedTab");
        if (!kit.c.has("search") || selected.getType() != CreativeModeTab.Type.SEARCH) return;
        var s = kit.c.getAsJsonObject("search");
        var box = (EditBox) Stub.get(screen, CreativeModeInventoryScreen.class, "searchBox");
        box.setValue(s.get("text").getAsString());
        Stub.method(CreativeModeInventoryScreen.class, "refreshSearchResults").invoke(screen);
        box.setCursorPosition(s.get("cursor").getAsInt());
        box.setHighlightPos(s.get("highlight").getAsInt());
        // Keeps the cursor's 300 ms blink phase away from its edges.
        Stub.put(box, EditBox.class, "focusedTime", Util.getMillis() - (s.get("blink").getAsInt() * 300L + 150L));
    }

    static void run(ScreenKit kit, JsonObject c) throws Exception {
        CreativeStubs.install(kit);
        // Initialized first, or its static initializer would overwrite the selected tab set here.
        Class.forName(CreativeModeInventoryScreen.class.getName(), true, CreativeModeInventoryScreen.class.getClassLoader());
        var tab = BuiltInRegistries.CREATIVE_MODE_TAB.getValue(Identifier.parse(c.get("tab").getAsString()));
        Stub.putStatic(CreativeModeInventoryScreen.class, "selectedTab", tab);
        var screen = new CreativeModeInventoryScreen(kit.player, FeatureFlags.DEFAULT_FLAGS, kit.bool("opTab", false));
        kit.open(screen);
        for (var e : kit.list("inventory")) {
            var a = e.getAsJsonArray();
            kit.inventory.setItem(a.get(0).getAsInt(), ScreenKit.stack(a.get(1)));
        }
        var menu = screen.getMenu();
        menu.setCarried(kit.key(ScreenKit.stack(c.get("carried")), -2));
        search(kit, screen);
        if (c.has("scroll") && menu.canScroll()) {
            Stub.put(screen, CreativeModeInventoryScreen.class, "scrollOffs", c.get("scroll").getAsFloat());
            menu.scrollTo(c.get("scroll").getAsFloat());
        }
        for (int i = 0; i < menu.slots.size(); i++) {
            var stack = menu.slots.get(i).getItem();
            if (!stack.isEmpty()) kit.capture.keys.put(stack, i);
        }
        var tabs = CreativeModeTabs.tabs();
        for (int i = 0; i < tabs.size(); i++) kit.capture.keys.put(tabs.get(i).getIconItem(), ICONS + i);
        kit.frame(screen);
        game(kit, screen);
    }

    static void game(ScreenKit kit, CreativeModeInventoryScreen screen) throws Exception {
        var tabs = new JsonArray();
        for (CreativeModeTab tab : CreativeModeTabs.tabs()) {
            var o = new JsonObject();
            o.addProperty("id", BuiltInRegistries.CREATIVE_MODE_TAB.getKey(tab).toString());
            o.addProperty("type", tab.getType().name());
            o.addProperty("row", tab.row().name());
            o.addProperty("column", tab.column());
            o.addProperty("right", tab.isAlignedRight());
            o.addProperty("showTitle", tab.showTitle());
            o.addProperty("canScroll", tab.canScroll());
            o.addProperty("background", tab.getBackgroundTexture().getPath());
            o.add("title", ComponentSerialization.CODEC.encodeStart(ScreenKit.ops(), tab.getDisplayName()).getOrThrow());
            tabs.add(o);
        }
        kit.game.add("tabs", tabs);
        var menu = screen.getMenu();
        kit.game.addProperty("menu", menu.items.size());
        var empty = new JsonArray();
        for (int i = 0; i < menu.items.size(); i++) if (menu.items.get(i).isEmpty()) empty.add(i);
        kit.game.add("empty", empty);

        var hovered = (Slot) Stub.get(screen, AbstractContainerScreen.class, "hoveredSlot");
        if (hovered == null || !hovered.hasItem()) return;
        var stack = hovered.getItem();
        kit.items.add(Integer.toString(menu.slots.indexOf(hovered)), ScreenKit.resolved(stack));
        var member = new JsonArray();
        var all = CreativeModeTabs.tabs();
        for (int i = 0; i < all.size(); i++) if (all.get(i).contains(stack)) member.add(i);
        kit.game.add("member", member);
        @SuppressWarnings("unchecked")
        var visible = (Set<TagKey<Item>>) Stub.get(screen, CreativeModeInventoryScreen.class, "visibleTags");
        var tags = new JsonArray();
        for (var tag : visible) tags.add(tag.location().toString());
        kit.game.add("visibleTags", tags);
        var own = new JsonArray();
        stack.tags().forEach(t -> own.add(t.location().toString()));
        kit.game.add("tags", own);
    }

    public static void main(String[] args) throws Exception {
        ScreenKit.run(args, CreativeOracle::run);
    }
}
