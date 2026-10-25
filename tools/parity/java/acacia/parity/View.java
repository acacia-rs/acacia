package acacia.parity;

import com.mojang.authlib.GameProfile;
import java.util.UUID;
import net.minecraft.client.Camera;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.ClientRecipeBook;
import net.minecraft.client.multiplayer.ClientLevel;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.client.multiplayer.chat.ChatAbilities;
import net.minecraft.client.player.ItemActivation;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.client.renderer.GameRenderer;
import net.minecraft.client.renderer.LightmapRenderStateExtractor;
import net.minecraft.client.renderer.fog.FogData;
import net.minecraft.client.renderer.fog.FogRenderer;
import net.minecraft.client.renderer.state.LightmapRenderState;
import net.minecraft.stats.StatsCounter;
import net.minecraft.world.entity.player.Input;

/**
 * The player's view as GameRenderer extracts it each frame: a real LocalPlayer, the main Camera
 * aligned to it (its EnvironmentAttributeProbe sampling the level's biomes and timelines), and
 * the lightmap and fog state extractors over them. GameRenderer and FogRenderer are shells: the
 * fields these paths read are set, nothing touches the GPU. The lightmap texture itself is
 * computed on the GPU (shaders/core/lightmap.fsh) from the LightmapRenderState returned here.
 */
public final class View {
    private static final UUID PLAYER_ID = new UUID(0L, 1L);

    private View() {}

    /** A fixed partial tick, as DeltaTracker.ZERO/ONE but for any value. */
    public static DeltaTracker partial(float partialTicks) {
        return new DeltaTracker() {
            @Override public float getGameTimeDeltaTicks() { return partialTicks; }
            @Override public float getGameTimeDeltaPartialTick(boolean ignoreFrozenGame) { return partialTicks; }
            @Override public float getRealtimeDeltaTicks() { return partialTicks; }
        };
    }

    /** A real LocalPlayer at (x, y, z) looking (yRot, xRot), added to `level` and set as Minecraft's player. */
    public static LocalPlayer player(ClientLevel level, double x, double y, double z, float yRot, float xRot) throws Exception {
        var mc = Client.minecraft();
        ClientPacketListener connection = Shell.get(level, "connection");
        Shell.set(connection, "localGameProfile", new GameProfile(PLAYER_ID, "parity"));
        var player = new LocalPlayer(mc, level, connection, new StatsCounter(), new ClientRecipeBook(), Input.EMPTY, false,
            ChatAbilities.NO_RESTRICTIONS, new ItemActivation());
        player.setId(Client.nextEntityId());
        player.snapTo(x, y, z, yRot, xRot);
        level.addEntity(player);
        Shell.set(mc, "player", player);
        return player;
    }

    /**
     * Minecraft's player's camera after a frame's Camera.update and a tick: alignWithEntity, fov
     * and hud fov, fluid in camera, attribute probe at the camera. Update's cull frustum and
     * projection need the GPU device and are skipped. Installed as the GameRenderer shell's mainCamera.
     */
    public static Camera camera(float partialTicks) throws Exception {
        var mc = Client.minecraft();
        var camera = new Camera();
        Shell.set(gameRenderer(), "mainCamera", camera);
        camera.setLevel(mc.level);
        camera.setEntity(mc.player);
        Class<?>[] f = {float.class};
        Shell.call(Camera.class, camera, "alignWithEntity", f, partialTicks);
        Shell.set(camera, "fov", (float) Shell.call(Camera.class, camera, "calculateFov", f, partialTicks));
        Shell.set(camera, "hudFov", (float) Shell.call(Camera.class, camera, "calculateHudFov", f, partialTicks));
        Shell.set(camera, "initialized", true);
        camera.tick();
        return camera;
    }

    /** GameRenderer.extractCamera's fog: FogRenderer.setupFog at the options' effective render distance. */
    public static FogData fog(Camera camera, float partialTicks) throws Exception {
        var mc = Client.minecraft();
        var renderer = Shell.alloc(FogRenderer.class);
        return renderer.setupFog(camera, mc.options.getEffectiveRenderDistance(), partial(partialTicks),
            gameRenderer().bossOverlayWorldDarkening(partialTicks), mc.level);
    }

    /**
     * The lightmap's CPU inputs as the frame extracts them, on the camera from `camera`. Block
     * light flicker starts at 0 and only moves on LightmapRenderStateExtractor.tick (random).
     */
    public static LightmapRenderState lightmap(float partialTicks) throws Exception {
        var extractor = new LightmapRenderStateExtractor(gameRenderer(), Client.minecraft());
        Shell.set(extractor, "needsUpdate", true);
        var state = new LightmapRenderState();
        extractor.extract(state, partialTicks);
        return state;
    }

    /** Minecraft's GameRenderer: a shell holding the main camera (and boss-overlay darkening 0). */
    public static synchronized GameRenderer gameRenderer() throws Exception {
        var mc = Client.minecraft();
        if (mc.gameRenderer == null) {
            var renderer = Shell.alloc(GameRenderer.class);
            Shell.set(renderer, "mainCamera", new Camera());
            Shell.set(mc, "gameRenderer", renderer);
        }
        return mc.gameRenderer;
    }
}
