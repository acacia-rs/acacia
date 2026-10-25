package acacia.parity.gui;

import static acacia.parity.gui.HudActor.b;
import static acacia.parity.gui.HudActor.f;
import static acacia.parity.gui.HudActor.i;

import com.google.gson.Gson;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Proxy;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import net.minecraft.client.AttackIndicatorStatus;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.Minecraft;
import net.minecraft.client.OptionInstance;
import net.minecraft.client.Options;
import net.minecraft.client.gui.Hud;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.multiplayer.MultiPlayerGameMode;
import net.minecraft.util.Util;
import net.minecraft.world.entity.EntityEquipment;
import net.minecraft.world.entity.player.Inventory;
import net.minecraft.world.food.FoodData;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;
import net.minecraft.world.level.GameType;

/**
 * The `hud` harness's oracle: vanilla's own Hud methods, BossHealthOverlay, JumpableVehicleBar,
 * ChatComponent and itemDecorations on a stand-in client whose player answers from the case,
 * recorded by GuiCapture. Case kinds: cases.py. Input lines: case JSON; output lines:
 * {case, draws, state...} or {case, error}. Adapted from Horizon's VanillaHudOracle (MIT,
 * (c) Ludovic Marie).
 */
public final class HudOracle {
    /** One case's stand-in client. */
    static final class Ctx {
        Minecraft mc;
        HudActor actor;
        ClientLevel level;
        Hud hud;
        Options options;
        net.minecraft.client.gui.Gui gui;
        net.minecraft.client.Camera camera;
        GuiCapture capture;
        DeltaTracker delta;
        Inventory inventory;
        net.minecraft.client.gui.components.ChatComponent chat;
        GameType mode;
        float partial;
        JsonObject result;
    }

    static <T> OptionInstance<T> option(T value) throws Exception {
        @SuppressWarnings("unchecked") OptionInstance<T> o = Stub.alloc(OptionInstance.class);
        Stub.put(o, OptionInstance.class, "value", value);
        return o;
    }

    static Ctx setUp(JsonObject c) throws Exception {
        var x = new Ctx();
        x.mc = Stub.alloc(Minecraft.class);
        x.actor = Stub.alloc(HudActor.class);
        x.level = Stub.alloc(ClientLevel.class);
        var levelData = Stub.alloc(ClientLevel.ClientLevelData.class);
        Stub.put(levelData, ClientLevel.ClientLevelData.class, "hardcore", b("hardcore"));
        Stub.put(x.level, ClientLevel.class, "clientLevelData", levelData);
        x.actor.level = x.level;
        x.actor.offhand = ItemStack.EMPTY;
        x.actor.active = new ArrayList<>();
        x.actor.food = new FoodData();
        x.actor.food.setFoodLevel(i("food", 20));
        x.actor.food.setSaturation(f("saturation", 5));
        x.actor.damageCooldownTime = i("damageCooldown", 0);
        x.actor.experienceProgress = f("xpProgress", 0);
        x.actor.experienceLevel = i("xpLevel", 0);
        x.inventory = new Inventory(x.actor, new EntityEquipment());
        Stub.put(x.actor, net.minecraft.world.entity.player.Player.class, "inventory", x.inventory);
        x.inventory.setSelectedSlot(i("selected", 0));
        Stub.put(x.mc, Minecraft.class, "player", x.actor);
        x.camera = Stub.alloc(net.minecraft.client.Camera.class);
        Stub.put(x.camera, net.minecraft.client.Camera.class, "entity", x.actor);
        var renderer = Stub.alloc(net.minecraft.client.renderer.GameRenderer.class);
        Stub.put(renderer, net.minecraft.client.renderer.GameRenderer.class, "mainCamera", x.camera);
        Stub.put(x.mc, Minecraft.class, "gameRenderer", renderer);
        Stub.put(x.mc, Minecraft.class, "font", GuiGame.font);
        x.mode = GameType.byName(c.has("gameMode") ? c.get("gameMode").getAsString() : "survival");
        var gameMode = Stub.alloc(MultiPlayerGameMode.class);
        Stub.put(gameMode, MultiPlayerGameMode.class, "localPlayerMode", x.mode);
        Stub.put(x.mc, Minecraft.class, "gameMode", gameMode);
        x.options = Stub.alloc(Options.class);
        Stub.put(x.options, Options.class, "attackIndicator", option(AttackIndicatorStatus.valueOf(c.has("attackIndicator") ? c.get("attackIndicator").getAsString() : "OFF")));
        Stub.put(x.options, Options.class, "backgroundForChatOnly", option(c.has("chatOnly") ? b("chatOnly") : true));
        Stub.put(x.options, Options.class, "textBackgroundOpacity", option((double) f("textOpacity", 0.5F)));
        Stub.put(x.options, Options.class, "notificationDisplayTime", option(c.has("displayTime") ? c.get("displayTime").getAsDouble() : 1.0));
        Stub.put(x.mc, Minecraft.class, "options", x.options);
        var window = Stub.alloc(com.mojang.blaze3d.platform.Window.class);
        Stub.put(window, com.mojang.blaze3d.platform.Window.class, "guiScaledWidth", i("width", 0));
        Stub.put(window, com.mojang.blaze3d.platform.Window.class, "guiScaledHeight", i("height", 0));
        Stub.put(x.mc, Minecraft.class, "window", window);
        x.gui = Stub.alloc(net.minecraft.client.gui.Gui.class);
        Stub.put(x.mc, Minecraft.class, "gui", x.gui);
        Stub.put(x.mc, Minecraft.class, "demo", b("demo"));
        x.hud = Stub.alloc(Hud.class);
        Stub.put(x.hud, Hud.class, "minecraft", x.mc);
        Stub.put(x.hud, Hud.class, "random", net.minecraft.util.RandomSource.create());
        Stub.put(x.hud, Hud.class, "tickCount", i("tick", 0));
        Stub.put(x.hud, Hud.class, "lastHealth", i("lastHealth", 0));
        Stub.put(x.hud, Hud.class, "displayHealth", i("displayHealth", 0));
        Stub.put(x.hud, Hud.class, "lastHealthTime", Util.getMillis() - (long) i("lastHealthAgo", 0));
        Stub.put(x.hud, Hud.class, "healthBlinkTime", (long) i("healthBlinkTime", 0));
        Stub.put(x.hud, Hud.class, "lastToolHighlight", ItemStack.EMPTY);
        x.chat = Stub.alloc(net.minecraft.client.gui.components.ChatComponent.class);
        Stub.put(x.chat, net.minecraft.client.gui.components.ChatComponent.class, "messageDeletionQueue", new ArrayList<>());
        Stub.put(x.hud, Hud.class, "chat", x.chat);
        x.hud.resetTitleTimes();
        x.capture = GuiCapture.create(x.mc, i("width", 0), i("height", 0));
        x.partial = f("partial", 0);
        float partial = x.partial;
        x.delta = (DeltaTracker) Proxy.newProxyInstance(DeltaTracker.class.getClassLoader(), new Class<?>[] {DeltaTracker.class},
            (proxy, method, a) -> method.getReturnType() == float.class ? partial : null);
        return x;
    }

    /** The hotbar's stacks (slot n holds the n-th item) and the off hand's totem, keyed by slot. */
    static void hotbarItems(Ctx x, JsonObject c) {
        var stacks = c.getAsJsonArray("slots");
        var items = new net.minecraft.world.item.Item[] {Items.STONE, Items.DIAMOND_SWORD, Items.APPLE, Items.TORCH, Items.BOW,
            Items.OAK_LOG, Items.BREAD, Items.SHIELD, Items.COMPASS};
        for (int n = 0; stacks != null && n < 9; n++) {
            if (stacks.get(n).isJsonNull()) continue;
            var stack = new ItemStack(items[n]);
            stack.setPopTime(stacks.get(n).getAsInt());
            x.inventory.setItem(n, stack);
            x.capture.keys.put(stack, n);
        }
        if (c.has("offhand") && !c.get("offhand").isJsonNull()) {
            x.actor.offhand = new ItemStack(Items.TOTEM_OF_UNDYING);
            x.actor.offhand.setPopTime(c.get("offhand").getAsInt());
            x.capture.keys.put(x.actor.offhand, 40);
        }
    }

    public static void main(String[] args) throws Exception {
        GuiGame.init();
        var gson = new Gson();
        var out = new StringBuilder();
        for (String line : Files.readAllLines(Path.of(args[0]))) {
            if (line.isBlank()) continue;
            var c = JsonParser.parseString(line).getAsJsonObject();
            HudActor.c = c;
            var result = new JsonObject();
            result.add("case", c);
            try {
                var x = setUp(c);
                x.result = result;
                hotbarItems(x, c);
                if (c.has("vehicleMaxHealth")) x.actor.vehicle = Stub.alloc(HudActor.Mount.class);
                HudKinds.run(c.get("kind").getAsString(), x, c);
                result.add("draws", x.capture.draws);
            } catch (Throwable error) {
                var cause = error instanceof InvocationTargetException && error.getCause() != null ? error.getCause() : error;
                var sw = new java.io.StringWriter();
                cause.printStackTrace(new java.io.PrintWriter(sw));
                result.addProperty("error", sw.toString().lines().limit(8).reduce("", (a, b) -> a + b + " | "));
            }
            out.append(gson.toJson(result)).append('\n');
        }
        Files.writeString(Path.of(args[1]), out.toString(), StandardCharsets.UTF_8);
    }
}
