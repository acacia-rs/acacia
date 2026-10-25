package acacia.parity;

import java.util.HashMap;
import java.util.List;
import java.util.Map;
import net.minecraft.client.renderer.texture.SpriteLoader;
import net.minecraft.client.renderer.texture.TextureAtlas;
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import net.minecraft.client.resources.model.sprite.AtlasManager;
import net.minecraft.client.resources.model.sprite.SpriteId;
import net.minecraft.data.AtlasIds;
import net.minecraft.resources.Identifier;

/**
 * The client's AtlasManager over UnitAtlas: every atlas it knows (blocks, items, gui, particles...),
 * each a TextureAtlas shell holding its sprites, so getAtlasOrThrow, getSprite and get(SpriteId)
 * answer as in game. No texture is uploaded; each atlas's view is a Textures.NamedView. The gui
 * atlas names its sprites (UnitAtlas nameSprites) so GUI draw records say which sprite they blit.
 */
public final class Atlases {
    private static AtlasManager manager;
    private static final Map<Identifier, SpriteLoader.Preparations> byId = new HashMap<>();
    private static final Map<Identifier, TextureAtlas> atlases = new HashMap<>();

    private Atlases() {}

    public static synchronized AtlasManager manager() throws Exception {
        if (manager != null) return manager;
        Game.init();
        List<AtlasManager.AtlasConfig> configs = Shell.get(AtlasManager.class, "KNOWN_ATLASES");
        Class<?> entryType = Class.forName(AtlasManager.class.getName() + "$AtlasEntry");
        Map<Identifier, Object> byTexture = new HashMap<>(), byDefinition = new HashMap<>();
        Map<SpriteId, TextureAtlasSprite> lookup = new HashMap<>();
        for (var config : configs) {
            boolean gui = config.definitionLocation().equals(AtlasIds.GUI);
            var prepared = UnitAtlas.load(config.definitionLocation(), config.textureId(), config.additionalMetadata(), gui);
            byId.put(config.definitionLocation(), prepared);
            var atlas = Shell.alloc(TextureAtlas.class);
            Shell.set(atlas, "location", config.textureId());
            Shell.set(atlas, "textureView", new Textures.NamedView(config.textureId()));
            atlases.put(config.textureId(), atlas);
            Shell.set(atlas, "texturesByName", Map.copyOf(prepared.regions()));
            Shell.set(atlas, "missingSprite", prepared.missing());
            Shell.set(atlas, "sprites", List.copyOf(prepared.regions().values()));
            Shell.set(atlas, "animatedTexturesStates", List.of());
            var entry = Shell.construct(entryType, atlas, config);
            byTexture.put(config.textureId(), entry);
            byDefinition.put(config.definitionLocation(), entry);
            prepared.regions().forEach((id, sprite) -> lookup.put(new SpriteId(config.textureId(), id), sprite));
        }
        var shell = Shell.alloc(AtlasManager.class);
        Shell.set(shell, "atlasByTexture", byTexture);
        Shell.set(shell, "atlasById", byDefinition);
        Shell.set(shell, "spriteLookup", lookup);
        manager = shell;
        return manager;
    }

    /** The atlas shells by texture id (TextureAtlas.LOCATION_BLOCKS...), as a TextureManager holds them. */
    public static Map<Identifier, TextureAtlas> byTexture() throws Exception {
        manager();
        return Map.copyOf(atlases);
    }

    /** One atlas's sprites by its definition id (AtlasIds.BLOCKS...). */
    public static SpriteLoader.Preparations prepared(Identifier atlasId) throws Exception {
        manager();
        return byId.get(atlasId);
    }
}
