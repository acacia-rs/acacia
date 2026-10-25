package acacia.parity.gui;

import static acacia.parity.gui.ScreenGame.b;
import static acacia.parity.gui.ScreenGame.component;
import static acacia.parity.gui.ScreenGame.f;
import static acacia.parity.gui.ScreenGame.i;
import static acacia.parity.gui.ScreenGame.s;

import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.mojang.renderpearl.api.textures.GpuTextureView;
import it.unimi.dsi.fastutil.ints.IntSet;
import java.util.HashMap;
import java.util.List;
import java.util.Optional;
import net.minecraft.client.Minecraft;
import net.minecraft.client.User;
import net.minecraft.client.gui.components.LogoRenderer;
import net.minecraft.client.gui.components.debug.DebugScreenEntryList;
import net.minecraft.client.gui.screens.CreditsAndAttributionScreen;
import net.minecraft.client.gui.screens.GenericMessageScreen;
import net.minecraft.client.gui.screens.LevelLoadingScreen;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.WinScreen;
import net.minecraft.client.multiplayer.LevelLoadTracker;
import net.minecraft.client.renderer.GameRenderer;
import net.minecraft.client.renderer.Panorama;
import net.minecraft.client.renderer.state.GameRenderState;
import net.minecraft.client.renderer.state.gui.GuiRenderState;
import net.minecraft.client.renderer.texture.AbstractTexture;
import net.minecraft.client.renderer.texture.SpriteContents;
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import net.minecraft.client.renderer.texture.TextureManager;
import net.minecraft.resources.Identifier;
import net.minecraft.server.level.progress.ChunkLoadStatusView;
import net.minecraft.server.packs.PackType;
import net.minecraft.server.packs.resources.ReloadableResourceManager;
import net.minecraft.server.packs.resources.Resource;
import net.minecraft.world.level.chunk.status.ChunkStatus;

/**
 * The `screens_more` harness's oracle: LevelLoadingScreen, GenericMessageScreen,
 * CreditsAndAttributionScreen and WinScreen built by `init` at the case's GUI size on the stand-in
 * client (in a world or not), ticked or scrolled, and drawn as
 * `extractRenderStateWithTooltipAndSubtitles` draws without the deferred tooltips. The panorama
 * holds still, the end portal's textures are stand-ins and the nether portal's sprite is named
 * without the block atlas. Case fields: cases.py.
 */
public final class MoreOracle {
    /** The tracker's answers come from the case; the level is never ready (that closes the screen). */
    public static final class Tracker extends LevelLoadTracker {
        boolean has;
        float progress;
        ChunkLoadStatusView view;
        @Override public boolean hasProgress() { return has; }
        @Override public float serverProgress() { return progress; }
        @Override public ChunkLoadStatusView statusView() { return view; }
        @Override public boolean isLevelReady() { return false; }
    }

    /** A texture with no GPU view: the end portal fill only passes it on. */
    public static final class NoTexture extends AbstractTexture {
        @Override public GpuTextureView getTextureView() { return null; }
    }

    /** The client jar's resources behind the Minecraft field's type. */
    public static final class Resources extends ReloadableResourceManager {
        public Resources() { super(PackType.CLIENT_RESOURCES); }
        @Override public Optional<Resource> getResource(Identifier id) { return GuiGame.resources.getResource(id); }
    }

    static final String[] STATUSES = {"EMPTY", "STRUCTURE_STARTS", "STRUCTURE_REFERENCES", "BIOMES", "TERRAIN", "FEATURES",
        "INITIALIZE_LIGHT", "LIGHT", "SPAWN", "FULL"};

    static ChunkLoadStatusView view(JsonObject v) throws Exception {
        int radius = i(v, "radius", 0);
        JsonArray cells = v.getAsJsonArray("cells");
        var statuses = new ChunkStatus[cells.size()];
        for (int n = 0; n < statuses.length; n++) {
            int s = cells.get(n).getAsInt();
            statuses[n] = s < 0 ? null : (ChunkStatus) ChunkStatus.class.getField(STATUSES[s]).get(null);
        }
        int d = radius * 2 + 1;
        return new ChunkLoadStatusView() {
            @Override public void moveTo(net.minecraft.resources.ResourceKey<net.minecraft.world.level.Level> dim, net.minecraft.world.level.ChunkPos pos) { }
            @Override public ChunkStatus get(int x, int z) { return statuses[x * d + z]; }
            @Override public int radius() { return radius; }
        };
    }

    static Screen loading(Minecraft mc, JsonObject c) throws Exception {
        var tracker = Stub.alloc(Tracker.class);
        tracker.has = b(c, "hasProgress", false);
        tracker.progress = f(c, "progress", 0.0F);
        tracker.view = c.has("view") ? view(c.getAsJsonObject("view")) : null;
        var debug = Stub.alloc(DebugScreenEntryList.class);
        var enabled = new java.util.ArrayList<Identifier>();
        if (b(c, "visualize", false)) enabled.add(Identifier.withDefaultNamespace("visualize_chunks_on_server"));
        Stub.put(debug, DebugScreenEntryList.class, "currentlyEnabled", enabled);
        Stub.put(mc, Minecraft.class, "debugEntries", debug);
        var screen = new LevelLoadingScreen(tracker, LevelLoadingScreen.Reason.valueOf(s(c, "reason", "OTHER")));
        var contents = Stub.alloc(SpriteContents.class);
        Stub.put(contents, SpriteContents.class, "name", Identifier.withDefaultNamespace("block/nether_portal"));
        var sprite = Stub.alloc(TextureAtlasSprite.class);
        Stub.put(sprite, TextureAtlasSprite.class, "contents", contents);
        Stub.put(screen, LevelLoadingScreen.class, "cachedNetherPortalSprite", sprite);
        return screen;
    }

    static WinScreen win(Minecraft mc, JsonObject c) throws Exception {
        var user = Stub.alloc(User.class);
        Stub.put(user, User.class, "name", s(c, "player", "Steve"));
        Stub.put(mc, Minecraft.class, "user", user);
        Stub.put(mc, Minecraft.class, "resourceManager", new Resources());
        var screen = new WinScreen(b(c, "poem", false), () -> { });
        var logo = Stub.get(screen, WinScreen.class, "logoRenderer");
        Stub.put(logo, LogoRenderer.class, "showEasterEgg", false);
        return screen;
    }

    /** Scroll state set as the keys would leave it, then the frame's partial tick. */
    static void scroll(WinScreen screen, JsonObject c) throws Exception {
        Stub.put(screen, WinScreen.class, "scroll", f(c, "scroll", 0.0F));
        Stub.put(screen, WinScreen.class, "speedupActive", b(c, "speedup", false));
        Stub.put(screen, WinScreen.class, "direction", b(c, "reverse", false) ? -1 : 1);
        var mods = (IntSet) Stub.get(screen, WinScreen.class, "speedupModifiers");
        for (int m = 0; m < i(c, "modifiers", 0); m++) mods.add(m == 0 ? 1073742048 : 1073742052);
        float speed = (float) Stub.method(WinScreen.class, "calculateScrollSpeed").invoke(screen);
        Stub.put(screen, WinScreen.class, "scrollSpeed", speed);
    }

    static void stubClient(Minecraft mc) throws Exception {
        var renderer = Stub.alloc(GameRenderer.class);
        Stub.put(renderer, GameRenderer.class, "panorama", Stub.alloc(Panorama.class));
        var state = Stub.alloc(GameRenderState.class);
        Stub.put(state, GameRenderState.class, "guiRenderState", Stub.alloc(GuiRenderState.class));
        Stub.put(renderer, GameRenderer.class, "gameRenderState", state);
        Stub.put(mc, Minecraft.class, "gameRenderer", renderer);
        var textures = Stub.alloc(TextureManager.class);
        var none = Stub.alloc(NoTexture.class);
        Stub.put(textures, TextureManager.class, "byPath", new HashMap<Identifier, AbstractTexture>() {
            @Override public AbstractTexture get(Object key) { return none; }
        });
        Stub.put(mc, Minecraft.class, "textureManager", textures);
    }

    public static void main(String[] args) throws Exception {
        ScreenGame.run(args, (c, result) -> {
            int width = i(c, "width", 427), height = i(c, "height", 240);
            var mc = ScreenGame.create(width, height, b(c, "inWorld", true));
            ScreenGame.set(mc.options.menuBackgroundBlurriness(), i(c, "blur", 5));
            stubClient(mc);
            Screen screen = switch (c.get("kind").getAsString()) {
                case "loading" -> loading(mc, c);
                case "message" -> new GenericMessageScreen(component(c.get("title")));
                case "credits" -> new CreditsAndAttributionScreen(null);
                case "win" -> win(mc, c);
                default -> throw new IllegalArgumentException(c.get("kind").getAsString());
            };
            screen.init(width, height);
            for (int t = 0; t < i(c, "ticks", 0); t++) screen.tick();
            if (screen instanceof WinScreen w) scroll(w, c);
            var capture = GuiCapture.create(mc, width, height);
            JsonArray mouse = c.getAsJsonArray("mouse");
            int mx = mouse.get(0).getAsInt(), my = mouse.get(1).getAsInt();
            float a = f(c, "partial", 0.0F);
            capture.nextStratum();
            screen.extractBackground(capture, mx, my, a);
            capture.nextStratum();
            screen.extractRenderState(capture, mx, my, a);
            return capture.draws;
        });
    }
}
