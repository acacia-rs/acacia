package acacia.parity.gui;

import com.google.gson.Gson;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import com.mojang.serialization.JsonOps;
import java.lang.reflect.InvocationTargetException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import net.minecraft.client.Minecraft;
import net.minecraft.client.Options;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.core.component.DataComponentMap;
import net.minecraft.resources.RegistryOps;
import net.minecraft.world.TickRateManager;
import net.minecraft.world.entity.EntityEquipment;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.ai.attributes.AttributeMap;
import net.minecraft.world.entity.player.Inventory;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.Level;

/**
 * A stand-in client for the screen oracles: Minecraft.getInstance() with the default font, real
 * Options, a level answering registry and tick-rate questions, a LocalPlayer with a real Inventory
 * and attributes, and a GuiCapture for the case's GUI size. Stacks come from the case as
 * ItemStack.CODEC JSON; each stack an oracle keys is also written, fully resolved (every component,
 * defaults included), to the probe's input, so acacia reads the same item data the game used.
 *
 * <p>run() is an oracle's main loop: arguments (out dir, cases file); writes vanilla.jsonl (lines
 * {case, draws, state...} or {case, error}) and probe.jsonl (lines {case..., items: {key: stack}}).
 */
public final class ScreenKit {
    public interface Kind {
        void run(ScreenKit kit, JsonObject c) throws Exception;
    }

    private static Options options;

    public final JsonObject c;
    public final Minecraft mc;
    public final LocalPlayer player;
    public final Inventory inventory;
    public final ClientLevel level;
    public final GuiCapture capture;
    public final JsonObject result = new JsonObject();
    /** Resolved stacks by key, written to the probe's input. */
    public final JsonObject items = new JsonObject();

    private ScreenKit(JsonObject c) throws Exception {
        this.c = c;
        mc = Stub.alloc(Minecraft.class);
        Stub.putStatic(Minecraft.class, "instance", mc);
        Stub.put(mc, Minecraft.class, "font", GuiGame.font);
        if (options == null) {
            Stub.put(mc, Minecraft.class, "fixerUpper", net.minecraft.util.datafix.DataFixers.getDataFixer());
            options = new Options(mc, Files.createTempDirectory("options").toFile());
        }
        options.advancedItemTooltips = bool("advanced", false);
        Stub.put(mc, Minecraft.class, "options", options);
        level = Stub.alloc(ClientLevel.class);
        Stub.put(level, Level.class, "registryAccess", GuiGame.access);
        Stub.put(level, ClientLevel.class, "tickRateManager", new TickRateManager());
        Stub.put(level, ClientLevel.class, "mapData", new java.util.HashMap<>());
        Stub.put(mc, Minecraft.class, "level", level);
        player = Stub.alloc(LocalPlayer.class);
        Stub.put(player, LivingEntity.class, "attributes", new AttributeMap(Player.createAttributes().build()));
        inventory = new Inventory(player, new EntityEquipment());
        Stub.put(player, Player.class, "inventory", inventory);
        Stub.put(mc, Minecraft.class, "player", player);
        var gui = Stub.alloc(net.minecraft.client.gui.Gui.class);
        Stub.put(gui, net.minecraft.client.gui.Gui.class, "hud", Stub.alloc(net.minecraft.client.gui.Hud.class));
        Stub.put(mc, Minecraft.class, "gui", gui);
        Stub.put(mc, Minecraft.class, "lastInputType", net.minecraft.client.InputType.MOUSE);
        capture = GuiCapture.create(mc, i("width"), i("height"));
    }

    public int i(String key) {
        return c.get(key).getAsInt();
    }

    public int i(String key, int fallback) {
        return c.has(key) && !c.get(key).isJsonNull() ? c.get(key).getAsInt() : fallback;
    }

    public float f(String key, float fallback) {
        return c.has(key) && !c.get(key).isJsonNull() ? c.get(key).getAsFloat() : fallback;
    }

    public boolean bool(String key, boolean fallback) {
        return c.has(key) && !c.get(key).isJsonNull() ? c.get(key).getAsBoolean() : fallback;
    }

    static RegistryOps<JsonElement> ops() {
        return RegistryOps.create(JsonOps.INSTANCE, GuiGame.lookup);
    }

    /** A case stack: ItemStack.CODEC JSON ({id, count, components}); null or absent is empty. */
    public static ItemStack stack(JsonElement json) {
        if (json == null || json.isJsonNull()) return ItemStack.EMPTY;
        return ItemStack.CODEC.parse(ops(), json).getOrThrow();
    }

    /** {id, count, components}: every component the stack has, defaults included. */
    public static JsonObject resolved(ItemStack stack) {
        var o = new JsonObject();
        o.addProperty("id", stack.getItemHolder().getRegisteredName());
        o.addProperty("count", stack.getCount());
        o.add("components", DataComponentMap.CODEC.encodeStart(ops(), stack.getComponents()).getOrThrow());
        return o;
    }

    /** Records `stack` under `key` for both sides; returns it. */
    public ItemStack key(ItemStack stack, int key) {
        if (!stack.isEmpty()) {
            capture.keys.put(stack, key);
            items.add(Integer.toString(key), resolved(stack));
        }
        return stack;
    }

    /** Screen.init(width, height) without the narration and focus it also does. */
    public <S extends Screen> S open(S screen) throws Exception {
        screen.width = i("width");
        screen.height = i("height");
        Stub.method(Screen.class, "init").invoke(screen);
        return screen;
    }

    /** What Screen.extractRenderStateWithTooltipAndSubtitles draws: background, contents, deferred tooltip. */
    public void frame(Screen screen) {
        int mx = c.getAsJsonArray("mouse").get(0).getAsInt();
        int my = c.getAsJsonArray("mouse").get(1).getAsInt();
        float partial = f("partial", 0);
        screen.extractBackground(capture, mx, my, partial);
        screen.extractRenderState(capture, mx, my, partial);
        capture.extractDeferredElements(mx, my, partial);
    }

    public static void run(String[] args, Kind kind) throws Exception {
        GuiGame.init();
        var gson = new Gson();
        var vanilla = new StringBuilder();
        var probe = new StringBuilder();
        for (String line : Files.readAllLines(Path.of(args[1]))) {
            if (line.isBlank()) continue;
            var c = JsonParser.parseString(line).getAsJsonObject();
            JsonObject result = new JsonObject();
            JsonObject items = new JsonObject();
            try {
                var kit = new ScreenKit(c);
                result = kit.result;
                items = kit.items;
                kind.run(kit, c);
                result.add("draws", kit.capture.draws);
            } catch (Throwable error) {
                var cause = error instanceof InvocationTargetException && error.getCause() != null ? error.getCause() : error;
                var sw = new java.io.StringWriter();
                cause.printStackTrace(new java.io.PrintWriter(sw));
                result = new JsonObject();
                result.addProperty("error", sw.toString().lines().limit(10).reduce("", (a, b) -> a + b + " | "));
            }
            result.add("case", c);
            vanilla.append(gson.toJson(result)).append('\n');
            var p = c.deepCopy();
            p.add("items", items);
            probe.append(gson.toJson(p)).append('\n');
        }
        Files.writeString(Path.of(args[0], "vanilla.jsonl"), vanilla.toString(), StandardCharsets.UTF_8);
        Files.writeString(Path.of(args[0], "probe.jsonl"), probe.toString(), StandardCharsets.UTF_8);
    }

    /** The list of a case array, empty when absent. */
    public ArrayList<JsonElement> list(String key) {
        var out = new ArrayList<JsonElement>();
        if (c.has(key) && c.get(key).isJsonArray()) c.getAsJsonArray(key).forEach(out::add);
        return out;
    }
}
