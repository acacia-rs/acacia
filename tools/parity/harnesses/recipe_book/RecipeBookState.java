package acacia.parity.gui;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.Random;
import java.util.Set;
import net.minecraft.client.ClientRecipeBook;
import net.minecraft.client.gui.components.EditBox;
import net.minecraft.client.gui.components.WidgetTooltipHolder;
import net.minecraft.client.gui.screens.recipebook.GhostSlots;
import net.minecraft.client.gui.screens.recipebook.OverlayRecipeComponent;
import net.minecraft.client.gui.screens.recipebook.RecipeBookComponent;
import net.minecraft.client.gui.screens.recipebook.RecipeBookPage;
import net.minecraft.client.gui.screens.recipebook.RecipeBookTabButton;
import net.minecraft.client.gui.screens.recipebook.RecipeButton;
import net.minecraft.client.gui.screens.recipebook.RecipeCollection;
import net.minecraft.client.gui.screens.recipebook.SearchRecipeBookCategory;
import net.minecraft.client.gui.components.AbstractWidget;
import net.minecraft.util.Util;
import net.minecraft.util.context.ContextMap;
import net.minecraft.world.inventory.Slot;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.TooltipFlag;
import net.minecraft.world.item.crafting.display.RecipeDisplayEntry;
import net.minecraft.world.item.crafting.display.RecipeDisplayId;
import net.minecraft.world.item.crafting.display.SlotDisplayContext;

/**
 * The recipe book's state for a case: the recipes it knows (a seeded sample of the book type's
 * recipes whose results have a one-line tooltip), set before the screen opens; then the selected
 * tab, page, animations, time, search box, an open overlay and a ghost recipe, set through the real
 * component's fields and methods; then every stack it shows, keyed, and the collections, tabs and
 * ghost slots as the probe's `game` data. Highlighting new recipes is left out: showing one sends a
 * packet; the case's animation times stand in for it.
 */
final class RecipeBookState {
    private RecipeBookState() {}

    static final Map<SearchRecipeBookCategory, List<RecipeDisplayEntry>> POOLS = new HashMap<>();
    static final Class<?> OVERLAY_BUTTON = cls("net.minecraft.client.gui.screens.recipebook.OverlayRecipeComponent$OverlayRecipeButton");

    static Class<?> cls(String name) {
        try { return Class.forName(name); } catch (ClassNotFoundException e) { throw new RuntimeException(e); }
    }

    static Object rec(Object record, String name) throws Exception {
        Method m = record.getClass().getDeclaredMethod(name);
        m.setAccessible(true);
        return m.invoke(record);
    }

    static boolean plain(ScreenKit kit, ItemStack s) {
        return s.getTooltipLines(net.minecraft.world.item.Item.TooltipContext.of(kit.level), null, TooltipFlag.Default.NORMAL).size() == 1;
    }

    /** The book type's recipes whose every result has a one-line tooltip, in id order. */
    static List<RecipeDisplayEntry> pool(ScreenKit kit, SearchRecipeBookCategory search) {
        return POOLS.computeIfAbsent(search, s -> {
            var context = SlotDisplayContext.fromLevel(kit.level);
            var holders = new ArrayList<>(ScreenKit.recipes().getRecipes());
            holders.sort(Comparator.comparing(h -> h.id().identifier().toString()));
            var out = new ArrayList<RecipeDisplayEntry>();
            for (var h : holders) {
                ScreenKit.recipes().listDisplaysForRecipe(h.id(), e -> {
                    if (s.includedCategories().contains(e.category()) && e.resultItems(context).stream().allMatch(i -> plain(kit, i))) out.add(e);
                });
            }
            return out;
        });
    }

    /** The case's `recipes` sample of the pool, known to a fresh book; open and filtering as the case says. */
    static List<RecipeDisplayEntry> book(ScreenKit kit, SearchRecipeBookCategory search, net.minecraft.world.inventory.RecipeBookType type) throws Exception {
        var pool = new ArrayList<>(pool(kit, search));
        java.util.Collections.shuffle(pool, new Random(kit.i("seed", 0)));
        var chosen = pool.subList(0, Math.min(kit.i("recipes", 0), pool.size()));
        var book = new ClientRecipeBook();
        chosen.forEach(book::add);
        book.rebuildCollections();
        book.setOpen(type, kit.bool("open", false));
        book.setFiltering(type, kit.bool("filtering", false));
        Stub.put(kit.player, net.minecraft.client.player.LocalPlayer.class, "recipeBook", book);
        return chosen;
    }

    /** The widget's tooltip shows this frame when hovered: its delay passed while hovered before. */
    static void settle(Object widget) throws Exception {
        if (widget == null) return;
        var holder = (WidgetTooltipHolder) Stub.get(widget, AbstractWidget.class, "tooltip");
        Stub.put(holder, WidgetTooltipHolder.class, "wasDisplayed", true);
        Stub.put(holder, WidgetTooltipHolder.class, "displayStartTime", Util.getMillis() - 100_000L);
    }

    static float anim(ScreenKit kit, String key, int i) {
        var a = kit.list(key);
        return i < a.size() ? a.get(i).getAsFloat() : 0;
    }

    /** The case's tab, page, animations, time, search box, overlay and ghost recipe on the open component. */
    @SuppressWarnings("unchecked")
    static void apply(ScreenKit kit, RecipeBookComponent<?> comp, List<RecipeDisplayEntry> chosen) throws Exception {
        if (kit.c.has("ghost") && !chosen.isEmpty()) comp.fillGhostRecipe(chosen.get(kit.i("ghost") % chosen.size()).display());
        if (!comp.isVisible()) return;
        var tabs = (List<RecipeBookTabButton>) Stub.get(comp, RecipeBookComponent.class, "tabButtons");
        var tab = tabs.get(Math.floorMod(kit.i("tab", 0), tabs.size()));
        if (tab != Stub.get(comp, RecipeBookComponent.class, "selectedTab")) {
            Stub.method(RecipeBookComponent.class, "replaceSelected", RecipeBookTabButton.class).invoke(comp, tab);
        }
        boolean filtering = kit.bool("filtering", false);
        Stub.method(RecipeBookComponent.class, "updateCollections", boolean.class, boolean.class).invoke(comp, true, filtering);
        var page = (RecipeBookPage) Stub.get(comp, RecipeBookComponent.class, "recipeBookPage");
        int total = (int) Stub.get(page, RecipeBookPage.class, "totalPages");
        Stub.put(page, RecipeBookPage.class, "currentPage", Math.max(0, Math.min(kit.i("page", 0), total - 1)));
        Stub.method(RecipeBookPage.class, "updateButtonsForPage").invoke(page);
        for (int i = 0; i < tabs.size(); i++) Stub.put(tabs.get(i), RecipeBookTabButton.class, "animationTime", anim(kit, "tabAnims", i));
        var buttons = (List<RecipeButton>) Stub.get(page, RecipeBookPage.class, "buttons");
        for (int i = 0; i < buttons.size(); i++) Stub.put(buttons.get(i), RecipeButton.class, "animationTime", anim(kit, "buttonAnims", i));
        Stub.put(comp, RecipeBookComponent.class, "time", kit.f("time", 0));
        settle(Stub.get(comp, RecipeBookComponent.class, "filterButton"));
        settle(Stub.get(page, RecipeBookPage.class, "forwardButton"));
        settle(Stub.get(page, RecipeBookPage.class, "backButton"));
        if (kit.c.has("search")) {
            var s = kit.c.getAsJsonObject("search");
            var box = (EditBox) Stub.get(comp, RecipeBookComponent.class, "searchBox");
            box.setValue(s.get("value").getAsString());
            Stub.put(box, EditBox.class, "displayPos", s.get("display").getAsInt());
            Stub.put(box, EditBox.class, "cursorPos", s.get("cursor").getAsInt());
            Stub.put(box, EditBox.class, "highlightPos", s.get("highlight").getAsInt());
            Stub.put(box, AbstractWidget.class, "focused", s.get("focused").getAsBoolean());
            Stub.put(box, EditBox.class, "focusedTime", Util.getMillis() - s.get("blink").getAsLong());
        }
        if (kit.c.has("overlay") && !kit.c.get("overlay").isJsonNull()) {
            var button = buttons.get(kit.i("overlay"));
            if (button.visible) {
                int xo = (int) Stub.method(RecipeBookComponent.class, "getXOrigin").invoke(comp);
                int yo = (int) Stub.method(RecipeBookComponent.class, "getYOrigin").invoke(comp);
                var overlay = (OverlayRecipeComponent) Stub.get(page, RecipeBookPage.class, "overlay");
                overlay.init(button.getCollection(), SlotDisplayContext.fromLevel(kit.level), filtering, button.getX(), button.getY(),
                    xo + 147 / 2, yo + 13 + 166 / 2, button.getWidth());
                kit.game.addProperty("overlay", kit.i("overlay"));
            }
        }
    }

    static int next;

    static int key(ScreenKit kit, ItemStack s) {
        Integer k = kit.capture.keys.get(s);
        if (k != null) return k;
        kit.key(s, next);
        return next++;
    }

    static JsonArray keys(ScreenKit kit, List<ItemStack> stacks) {
        var a = new JsonArray();
        for (var s : stacks) if (!s.isEmpty()) a.add(key(kit, s));
        return a;
    }

    /** The component's tabs, collections and ghost slots, every stack keyed, into `kit.game`. */
    @SuppressWarnings("unchecked")
    static void export(ScreenKit kit, RecipeBookComponent<?> comp) throws Exception {
        next = 1000;
        var game = kit.game;
        var book = (ClientRecipeBook) Stub.get(comp, RecipeBookComponent.class, "book");
        var displays = new HashMap<RecipeDisplayId, JsonArray>();
        var cells = new HashMap<RecipeDisplayId, JsonArray>();
        var tabs = (List<RecipeBookTabButton>) Stub.get(comp, RecipeBookComponent.class, "tabButtons");
        if (comp.isVisible()) {
            var page = (RecipeBookPage) Stub.get(comp, RecipeBookComponent.class, "recipeBookPage");
            for (var b : (List<RecipeButton>) Stub.get(page, RecipeBookPage.class, "buttons")) {
                if (!b.visible) continue;
                for (Object e : (List<Object>) Stub.get(b, RecipeButton.class, "selectedEntries")) {
                    displays.put((RecipeDisplayId) rec(e, "id"), keys(kit, (List<ItemStack>) rec(e, "displayItems")));
                }
            }
            var overlay = (OverlayRecipeComponent) Stub.get(page, RecipeBookPage.class, "overlay");
            for (Object o : (List<Object>) Stub.get(overlay, OverlayRecipeComponent.class, "recipeButtons")) {
                var a = new JsonArray();
                for (Object pos : (List<Object>) Stub.get(o, OVERLAY_BUTTON, "slots")) {
                    var cell = new JsonArray();
                    cell.add((int) rec(pos, "x"));
                    cell.add((int) rec(pos, "y"));
                    cell.add(keys(kit, (List<ItemStack>) rec(pos, "ingredients")));
                    a.add(cell);
                }
                cells.put((RecipeDisplayId) Stub.get(o, OVERLAY_BUTTON, "recipe"), a);
            }
        }
        var index = new IdentityHashMap<RecipeCollection, Integer>();
        var collections = new JsonArray();
        var tabsJson = new JsonArray();
        for (var t : tabs) {
            var info = (RecipeBookComponent.TabInfo) Stub.get(t, RecipeBookTabButton.class, "tabInfo");
            var o = new JsonObject();
            o.addProperty("search", info.category() instanceof SearchRecipeBookCategory);
            var icons = new JsonArray();
            icons.add(key(kit, info.primaryIcon()));
            info.secondaryIcon().ifPresent(s -> icons.add(key(kit, s)));
            o.add("icons", icons);
            var list = new JsonArray();
            for (var c : book.getCollection(info.category())) {
                Integer n = index.get(c);
                if (n == null) {
                    n = collections.size();
                    index.put(c, n);
                    collections.add(collection(c, displays, cells));
                }
                list.add(n);
            }
            o.add("collections", list);
            tabsJson.add(o);
        }
        game.add("tabs", tabsJson);
        game.add("collections", collections);
        game.addProperty("selectedTab", tabs.indexOf(Stub.get(comp, RecipeBookComponent.class, "selectedTab")));
        var ghost = new JsonArray();
        var ghosts = (Map<Slot, Object>) Stub.get(Stub.get(comp, RecipeBookComponent.class, "ghostSlots"), GhostSlots.class, "ingredients");
        for (var e : ghosts.entrySet()) {
            var g = new JsonObject();
            g.addProperty("slot", e.getKey().index);
            g.addProperty("result", (boolean) rec(e.getValue(), "isResultSlot"));
            g.add("items", keys(kit, (List<ItemStack>) rec(e.getValue(), "items")));
            ghost.add(g);
        }
        game.add("ghost", ghost);
    }

    @SuppressWarnings("unchecked")
    static JsonObject collection(RecipeCollection c, Map<RecipeDisplayId, JsonArray> displays, Map<RecipeDisplayId, JsonArray> cells) throws Exception {
        var selected = (Set<RecipeDisplayId>) Stub.get(c, RecipeCollection.class, "selected");
        var craftable = (Set<RecipeDisplayId>) Stub.get(c, RecipeCollection.class, "craftable");
        var entries = new JsonArray();
        for (var e : c.getRecipes()) {
            var o = new JsonObject();
            o.addProperty("selected", selected.contains(e.id()));
            o.addProperty("craftable", craftable.contains(e.id()));
            if (displays.containsKey(e.id())) o.add("display", displays.get(e.id()));
            if (cells.containsKey(e.id())) o.add("cells", cells.get(e.id()));
            entries.add(o);
        }
        var o = new JsonObject();
        o.add("entries", entries);
        return o;
    }
}
