package acacia.parity.gui;

import com.google.gson.JsonObject;
import net.minecraft.client.gui.screens.inventory.AbstractContainerScreen;
import net.minecraft.client.gui.screens.inventory.AbstractRecipeBookScreen;
import net.minecraft.client.gui.screens.inventory.BlastFurnaceScreen;
import net.minecraft.client.gui.screens.inventory.CraftingScreen;
import net.minecraft.client.gui.screens.inventory.FurnaceScreen;
import net.minecraft.client.gui.screens.inventory.InventoryScreen;
import net.minecraft.client.gui.screens.inventory.SmokerScreen;
import net.minecraft.client.gui.screens.recipebook.RecipeBookComponent;
import net.minecraft.client.gui.screens.recipebook.SearchRecipeBookCategory;
import net.minecraft.network.chat.Component;
import net.minecraft.world.entity.player.Abilities;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.inventory.AbstractContainerMenu;
import net.minecraft.world.inventory.BlastFurnaceMenu;
import net.minecraft.world.inventory.CraftingMenu;
import net.minecraft.world.inventory.FurnaceMenu;
import net.minecraft.world.inventory.InventoryMenu;
import net.minecraft.world.inventory.RecipeBookType;
import net.minecraft.world.inventory.Slot;
import net.minecraft.world.inventory.SmokerMenu;

/**
 * The `recipe_book` harness's oracle: a real AbstractRecipeBookScreen (crafting table, furnace,
 * blast furnace, smoker, player inventory) over its real menu, the player's recipe book knowing a
 * sample of recipes (RecipeBookState), the slots and carried stack from the case, opened, the book
 * put in the case's state, then one frame; `book_*` kinds draw the component alone
 * (RecipeBookComponent.extractRenderState and extractTooltip). Adapted from Horizon's
 * VanillaRecipeBookScreenOracle, VanillaRecipeBookViewOracle and VanillaGhostRecipeOracle (MIT, (c) Ludovic Marie).
 */
public final class RecipeBookOracle {
    private RecipeBookOracle() {}

    static SearchRecipeBookCategory search(String kind) {
        return switch (kind) {
            case "furnace" -> SearchRecipeBookCategory.FURNACE;
            case "blast_furnace" -> SearchRecipeBookCategory.BLAST_FURNACE;
            case "smoker" -> SearchRecipeBookCategory.SMOKER;
            default -> SearchRecipeBookCategory.CRAFTING;
        };
    }

    static RecipeBookType type(String kind) {
        return switch (kind) {
            case "furnace" -> RecipeBookType.FURNACE;
            case "blast_furnace" -> RecipeBookType.BLAST_FURNACE;
            case "smoker" -> RecipeBookType.SMOKER;
            default -> RecipeBookType.CRAFTING;
        };
    }

    static AbstractRecipeBookScreen<?> create(ScreenKit kit, String kind) throws Exception {
        var inv = kit.inventory;
        return switch (kind) {
            case "crafting" -> new CraftingScreen(new CraftingMenu(0, inv), inv, Component.translatable("container.crafting"));
            case "furnace" -> new FurnaceScreen(new FurnaceMenu(0, inv), inv, Component.translatable("container.furnace"));
            case "blast_furnace" -> new BlastFurnaceScreen(new BlastFurnaceMenu(0, inv), inv, Component.translatable("container.blast_furnace"));
            case "smoker" -> new SmokerScreen(new SmokerMenu(0, inv), inv, Component.translatable("container.smoker"));
            case "inventory" -> {
                Stub.put(kit.player, Player.class, "inventoryMenu", new InventoryMenu(inv, false, kit.player));
                yield new InventoryScreen(kit.player);
            }
            default -> throw new IllegalArgumentException(kind);
        };
    }

    /** ContainerOracle.fill without the drag: slot contents keyed by index, the carried stack by -2. */
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
        menu.setCarried(kit.key(ScreenKit.stack(kit.c.get("carried")), -2));
        int id = 0;
        for (var v : kit.list("data")) menu.setData(id++, v.getAsInt());
    }

    static void run(ScreenKit kit, JsonObject c) throws Exception {
        String kind = c.get("kind").getAsString();
        boolean bare = kind.startsWith("book_");
        if (bare) kind = kind.substring(5);
        Stub.put(kit.player, Player.class, "abilities", new Abilities());
        RecipeBookStubs.install(kit);
        if (kind.equals("inventory")) kit.applyEffects();
        var chosen = RecipeBookState.book(kit, search(kind), type(kind));
        var screen = create(kit, kind);
        fill(kit, screen);
        Stub.put(kit.mc.gui, net.minecraft.client.gui.Gui.class, "screen", screen);
        kit.open(screen);
        var comp = (RecipeBookComponent<?>) Stub.get(screen, AbstractRecipeBookScreen.class, "recipeBookComponent");
        RecipeBookState.apply(kit, comp, chosen);
        RecipeBookState.export(kit, comp);
        if (kind.equals("inventory") && c.has("last")) {
            Stub.put(screen, InventoryScreen.class, "xMouse", c.getAsJsonArray("last").get(0).getAsFloat());
            Stub.put(screen, InventoryScreen.class, "yMouse", c.getAsJsonArray("last").get(1).getAsFloat());
        }
        if (!bare) {
            kit.frame(screen);
            return;
        }
        int mx = c.getAsJsonArray("mouse").get(0).getAsInt();
        int my = c.getAsJsonArray("mouse").get(1).getAsInt();
        float partial = kit.f("partial", 0);
        comp.extractRenderState(kit.capture, mx, my, partial);
        comp.extractTooltip(kit.capture, mx, my, null);
        kit.capture.extractDeferredElements(mx, my, partial);
    }

    public static void main(String[] args) throws Exception {
        ScreenKit.run(args, RecipeBookOracle::run);
    }
}
