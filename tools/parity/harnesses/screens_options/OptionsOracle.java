package acacia.parity.gui;

import static acacia.parity.gui.ScreenGame.b;
import static acacia.parity.gui.ScreenGame.i;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.mojang.authlib.minecraft.UserApiService;
import com.mojang.blaze3d.audio.DeviceList;
import com.mojang.blaze3d.audio.DeviceTracker;
import com.mojang.blaze3d.platform.Monitor;
import com.mojang.blaze3d.platform.MonitorManager;
import com.mojang.blaze3d.platform.Window;
import java.lang.reflect.Proxy;
import java.util.EnumSet;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.CompletableFuture;
import net.minecraft.client.Minecraft;
import net.minecraft.client.OptionInstance;
import net.minecraft.client.Options;
import net.minecraft.client.gui.components.OptionsList;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.options.AccessibilityOptionsScreen;
import net.minecraft.client.gui.screens.options.ChatOptionsScreen;
import net.minecraft.client.gui.screens.options.FontOptionsScreen;
import net.minecraft.client.gui.screens.options.MouseSettingsScreen;
import net.minecraft.client.gui.screens.options.OnlineOptionsScreen;
import net.minecraft.client.gui.screens.options.OptionsScreen;
import net.minecraft.client.gui.screens.options.OptionsSubScreen;
import net.minecraft.client.gui.screens.options.SkinCustomizationScreen;
import net.minecraft.client.gui.screens.options.SoundOptionsScreen;
import net.minecraft.client.gui.screens.options.VideoSettingsScreen;
import net.minecraft.client.gui.screens.options.controls.ControlsScreen;
import net.minecraft.client.gui.screens.social.PlayerSocialManager;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.client.renderer.GpuWarnlistManager;
import net.minecraft.client.sounds.SoundEngine;
import net.minecraft.client.sounds.SoundManager;
import net.minecraft.server.packs.repository.Pack;
import net.minecraft.server.packs.repository.PackRepository;
import net.minecraft.sounds.SoundSource;
import net.minecraft.world.entity.player.PlayerModelPart;
import net.minecraft.world.flag.FeatureFlagSet;
import net.minecraft.world.flag.FeatureFlags;

/**
 * The `screens_options` harness's oracle: `OptionsScreen` or one of its option-list pages built by
 * `init` on the stand-in client (in a world, no monitor, two sound devices) with the case's option
 * values, the list scrolled, optionally ticked, and drawn as `extractRenderStateWithTooltipAndSubtitles`
 * draws without the deferred tooltips. Case fields: cases.py.
 */
public final class OptionsOracle {
    /** No monitor: `SDL_GetDisplayForWindow` needs a real window. Allocated without its constructor. */
    public static final class NoMonitors extends MonitorManager {
        @Override public Monitor findBestMonitor(Window window) { return null; }
    }

    static OptionInstance<?> option(Options o, String id) throws Exception {
        if (id.startsWith("sound.")) {
            for (SoundSource s : SoundSource.values()) if (s.getName().equals(id.substring(6))) return o.getSoundSourceOptionInstance(s);
        }
        return (OptionInstance<?>) Options.class.getMethod(id).invoke(o);
    }

    @SuppressWarnings({"unchecked", "rawtypes"})
    static Object convert(Object current, JsonElement v) {
        if (current instanceof Boolean) return v.getAsBoolean();
        if (current instanceof Integer) return v.getAsInt();
        if (current instanceof Double) return v.getAsDouble();
        if (current instanceof String) return v.getAsString();
        return Enum.valueOf(((Enum) current).getDeclaringClass(), v.getAsString());
    }

    @SuppressWarnings("unchecked")
    static void setValues(Options o, JsonObject values) throws Exception {
        var parts = (Set<PlayerModelPart>) Stub.get(o, Options.class, "modelParts");
        for (var e : values.entrySet()) {
            if (e.getKey().startsWith("modelPart.")) {
                var part = PlayerModelPart.valueOf(e.getKey().substring(10).toUpperCase(java.util.Locale.ROOT));
                if (e.getValue().getAsBoolean()) parts.add(part); else parts.remove(part);
                continue;
            }
            var option = (OptionInstance<Object>) option(o, e.getKey());
            ScreenGame.set(option, convert(option.get(), e.getValue()));
        }
    }

    static void stubClient(Minecraft mc, JsonObject c, int width, int height) throws Exception {
        Stub.put(mc, Minecraft.class, "running", true);
        Window window = mc.getWindow();
        Stub.put(window, Window.class, "framebufferWidth", width * 2);
        Stub.put(window, Window.class, "framebufferHeight", height * 2);
        Stub.put(window, Window.class, "monitorManager", Stub.alloc(NoMonitors.class));
        Stub.put(mc, Minecraft.class, "gpuWarnlistManager", Stub.alloc(GpuWarnlistManager.class));
        var flags = b(c, "telemetry", false) ? Set.of(UserApiService.UserFlag.TELEMETRY_ENABLED) : Set.<UserApiService.UserFlag>of();
        Stub.put(mc, Minecraft.class, "userPropertiesFuture", CompletableFuture.completedFuture(new UserApiService.UserProperties(flags, Map.of())));
        var engine = Stub.alloc(SoundEngine.class);
        Object tracker = Proxy.newProxyInstance(OptionsOracle.class.getClassLoader(), new Class<?>[] {DeviceTracker.class},
            (proxy, method, args) -> method.getName().equals("currentDevices") ? new DeviceList(null, List.of("OpenAL Soft on Speakers", "Headphones")) : null);
        Stub.put(engine, SoundEngine.class, "deviceTracker", tracker);
        var sounds = Stub.alloc(SoundManager.class);
        Stub.put(sounds, SoundManager.class, "soundEngine", engine);
        Stub.put(mc, Minecraft.class, "soundManager", sounds);
        var packs = Stub.alloc(PackRepository.class);
        var available = new HashMap<String, Pack>();
        if (b(c, "highContrastPack", false)) available.put("high_contrast", null);
        Stub.put(packs, PackRepository.class, "available", available);
        Stub.put(mc, Minecraft.class, "resourcePackRepository", packs);
        var connection = Stub.alloc(ClientPacketListener.class);
        Stub.put(connection, ClientPacketListener.class, "enabledFeatures",
            b(c, "minecart", false) ? FeatureFlagSet.of(FeatureFlags.MINECART_IMPROVEMENTS) : FeatureFlagSet.of());
        Stub.put(mc.level, ClientLevel.class, "connection", connection);
        var social = Stub.alloc(PlayerSocialManager.class);
        Stub.put(social, PlayerSocialManager.class, "friendListEnabled", b(c, "friends", false));
        Stub.put(social, PlayerSocialManager.class, "allowFriendRequests", b(c, "friendRequests", false));
        Stub.put(mc, Minecraft.class, "playerSocialManager", social);
    }

    static Screen screen(Minecraft mc, String kind) {
        Options o = mc.options;
        return switch (kind) {
            case "options" -> new OptionsScreen(null, o);
            case "video" -> new VideoSettingsScreen(null, mc, o);
            case "sound" -> new SoundOptionsScreen(null, o);
            case "controls" -> new ControlsScreen(null, o);
            case "mouse" -> new MouseSettingsScreen(null, o);
            case "chat" -> new ChatOptionsScreen(null, o);
            case "accessibility" -> new AccessibilityOptionsScreen(null, o);
            case "skin" -> new SkinCustomizationScreen(null, o);
            case "font" -> new FontOptionsScreen(null, o);
            case "online" -> new OnlineOptionsScreen(null, o);
            default -> throw new IllegalArgumentException(kind);
        };
    }

    public static void main(String[] args) throws Exception {
        ScreenGame.run(args, (c, result) -> {
            int width = i(c, "width", 427), height = i(c, "height", 240);
            var mc = ScreenGame.create(width, height, true);
            stubClient(mc, c, width, height);
            setValues(mc.options, c.getAsJsonObject("values"));
            var screen = screen(mc, c.get("kind").getAsString());
            screen.init(width, height);
            if (screen instanceof OptionsSubScreen sub) {
                var list = (OptionsList) Stub.get(sub, OptionsSubScreen.class, "list");
                list.setScrollAmount(i(c, "scroll", 0));
            }
            if (b(c, "tick", false)) screen.tick();
            var capture = GuiCapture.create(mc, width, height);
            JsonArray mouse = c.getAsJsonArray("mouse");
            int mx = mouse.get(0).getAsInt(), my = mouse.get(1).getAsInt();
            capture.nextStratum();
            screen.extractBackground(capture, mx, my, 0.0F);
            capture.nextStratum();
            screen.extractRenderState(capture, mx, my, 0.0F);
            return capture.draws;
        });
    }
}
