package acacia.parity.gui;

import com.google.gson.JsonArray;
import com.mojang.blaze3d.platform.Lighting;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.components.AbstractWidget;
import net.minecraft.client.gui.screens.inventory.LoomScreen;
import net.minecraft.client.gui.screens.inventory.MerchantScreen;
import net.minecraft.client.gui.screens.inventory.StonecutterScreen;
import net.minecraft.client.model.geom.EntityModelSet;
import net.minecraft.client.renderer.GameRenderer;
import net.minecraft.client.renderer.Sheets;
import net.minecraft.client.resources.model.ModelManager;
import net.minecraft.network.chat.Component;
import net.minecraft.world.inventory.LoomMenu;
import net.minecraft.world.inventory.MerchantMenu;
import net.minecraft.world.inventory.StonecutterMenu;
import net.minecraft.world.item.crafting.display.SlotDisplayContext;
import net.minecraft.world.item.trading.MerchantOffer;
import net.minecraft.world.item.trading.MerchantOffers;

/** The trading harness's screens, each built, filled and set up as its case says. */
final class TradingScreens {
    private TradingScreens() {}

    /** An offer's stacks are keyed from here: baseCostA, costA, costB, result. */
    static final int OFFERS = 1000;
    /** A stonecutter recipe's icon is keyed RECIPES + its index. */
    static final int RECIPES = 2000;

    private static EntityModelSet models;

    /** LoomScreen sets up item lighting after its background: a Lighting that does nothing. */
    static final class NoLighting extends Lighting {
        @Override public void setupFor(Lighting.Entry entry) {}
    }

    static void merchant(TradingOracle o, ScreenKit kit) throws Exception {
        var menu = new MerchantMenu(0, kit.inventory);
        var offers = new MerchantOffers();
        for (var e : kit.list("offers")) offers.add(MerchantOffer.CODEC.parse(ScreenKit.ops(), e).getOrThrow());
        menu.setOffers(offers);
        menu.setXp(kit.i("xp", 0));
        menu.setMerchantLevel(kit.i("level", 0));
        menu.setShowProgressBar(kit.bool("bar", false));
        menu.setCanRestock(kit.bool("restock", false));
        var screen = kit.open(new MerchantScreen(menu, kit.inventory, o.title(Component.translatable("entity.minecraft.villager"))));
        o.fill(menu);
        o.keySlots(menu);
        for (int i = 0; i < offers.size(); i++) {
            var offer = offers.get(i);
            kit.key(offer.getBaseCostA(), OFFERS + 4 * i);
            o.byContent(offer.getCostA(), OFFERS + 4 * i + 1);
            kit.key(offer.getCostB(), OFFERS + 4 * i + 2);
            kit.key(offer.getResult(), OFFERS + 4 * i + 3);
        }
        Stub.put(screen, MerchantScreen.class, "shopItem", kit.i("shop", 0));
        Stub.put(screen, MerchantScreen.class, "scrollOff", kit.i("scroll", 0));
        // A button shows from the frame after the offers arrive; this is that frame.
        var buttons = (Object[]) Stub.get(screen, MerchantScreen.class, "tradeOfferButtons");
        for (int i = 0; i < buttons.length; i++) ((AbstractWidget) buttons[i]).visible = i < offers.size();
        o.menuState().addProperty("futureXp", menu.getFutureTraderXp());
        kit.frame(screen);
    }

    static void stonecutter(TradingOracle o, ScreenKit kit) throws Exception {
        var menu = new StonecutterMenu(0, kit.inventory);
        var screen = kit.open(new StonecutterScreen(menu, kit.inventory, o.title(Component.translatable("container.stonecutter"))));
        o.fill(menu);
        menu.slots.get(0).set(ScreenKit.stack(kit.c.get("input")));
        if (kit.c.has("select")) menu.clickMenuButton(kit.player, kit.i("select"));
        o.keySlots(menu);
        var recipes = menu.getVisibleRecipes().entries();
        var context = SlotDisplayContext.fromLevel(kit.level);
        for (int i = 0; i < recipes.size(); i++) {
            o.byContent(recipes.get(i).recipe().optionDisplay().resolveForFirstStack(context), RECIPES + i);
        }
        var state = o.menuState();
        state.addProperty("selected", menu.getSelectedRecipeIndex());
        state.addProperty("recipes", recipes.size());
        o.scroll(screen);
        kit.frame(screen);
    }

    static void loom(TradingOracle o, ScreenKit kit) throws Exception {
        var gameRenderer = Stub.alloc(GameRenderer.class);
        Stub.put(gameRenderer, GameRenderer.class, "lighting", Stub.alloc(NoLighting.class));
        Stub.put(kit.mc, Minecraft.class, "gameRenderer", gameRenderer);
        if (models == null) models = EntityModelSet.vanilla();
        var modelManager = Stub.alloc(ModelManager.class);
        Stub.put(modelManager, ModelManager.class, "entityModelSet", models);
        Stub.put(kit.mc, Minecraft.class, "modelManager", modelManager);

        var menu = new LoomMenu(0, kit.inventory);
        var screen = kit.open(new LoomScreen(menu, kit.inventory, o.title(Component.translatable("container.loom"))));
        o.fill(menu);
        menu.slots.get(0).set(ScreenKit.stack(kit.c.get("banner")));
        menu.slots.get(1).set(ScreenKit.stack(kit.c.get("dye")));
        menu.slots.get(2).set(ScreenKit.stack(kit.c.get("pattern")));
        if (kit.c.has("select")) menu.clickMenuButton(kit.player, kit.i("select"));
        o.keySlots(menu);
        var patterns = new JsonArray();
        for (var p : menu.getSelectablePatterns()) {
            patterns.add(TradingOracle.strings(p.value().translationKey(), Sheets.getBannerSprite(p).texture().toString()));
        }
        var state = o.menuState();
        state.addProperty("selected", menu.getSelectedBannerPatternIndex());
        state.add("patterns", patterns);
        o.scroll(screen);
        kit.frame(screen);
    }
}
