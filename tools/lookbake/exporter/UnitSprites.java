// SPDX-License-Identifier: MIT
// After tools/parity/java/acacia/parity/UnitAtlas.java.
package acacia.lookbake;

import java.util.HashMap;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.CompletableFuture;
import net.minecraft.client.renderer.Sheets;
import net.minecraft.client.renderer.texture.MissingTextureAtlasSprite;
import net.minecraft.client.renderer.texture.SpriteContents;
import net.minecraft.client.renderer.texture.SpriteLoader;
import net.minecraft.client.renderer.texture.TextureAtlas;
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import net.minecraft.client.renderer.texture.atlas.SpriteResourceLoader;
import net.minecraft.client.renderer.texture.atlas.SpriteSourceList;
import net.minecraft.client.resources.model.sprite.SpriteGetter;
import net.minecraft.client.resources.model.sprite.SpriteId;
import net.minecraft.data.AtlasIds;
import net.minecraft.resources.Identifier;
import net.minecraft.server.packs.resources.ResourceManager;

/**
 * The client's atlases as AtlasManager loads them (sources, images, metadata), each sprite placed
 * alone in a unit atlas: texture coordinates come out sprite-local in [0, 1] and a sprite's name
 * is its image (`minecraft:item/apple` is textures/item/apple.png). The stitcher is skipped.
 */
final class UnitSprites implements SpriteGetter {
    /** AtlasManager.KNOWN_ATLASES that items draw from: sheet texture to atlas definition. */
    private static final Map<Identifier, Identifier> ATLASES = Map.of(
        Sheets.BANNER_SHEET, AtlasIds.BANNER_PATTERNS,
        TextureAtlas.LOCATION_BLOCKS, AtlasIds.BLOCKS,
        TextureAtlas.LOCATION_ITEMS, AtlasIds.ITEMS,
        Sheets.CHEST_SHEET, AtlasIds.CHESTS,
        Sheets.DECORATED_POT_SHEET, AtlasIds.DECORATED_POT,
        Sheets.SHIELD_SHEET, AtlasIds.SHIELD_PATTERNS,
        Sheets.SHULKER_SHEET, AtlasIds.SHULKER_BOXES);

    private static final class Sprite extends TextureAtlasSprite {
        Sprite(Identifier atlas, SpriteContents contents) {
            super(atlas, contents, contents.width(), contents.height(), 0, 0, 0);
        }
    }

    private final ResourceManager resources;
    private final Map<Identifier, SpriteLoader.Preparations> loaded = new HashMap<>();

    UnitSprites(ResourceManager resources) {
        this.resources = resources;
    }

    /** `sheet` as in TextureAtlas.LOCATION_* or Sheets.*_SHEET. */
    synchronized SpriteLoader.Preparations atlas(Identifier sheet) {
        return loaded.computeIfAbsent(sheet, this::load);
    }

    private SpriteLoader.Preparations load(Identifier sheet) {
        var definition = ATLASES.get(sheet);
        if (definition == null) throw new IllegalArgumentException("no atlas " + sheet);
        var loader = SpriteResourceLoader.create(Set.of());
        Map<Identifier, TextureAtlasSprite> regions = new HashMap<>();
        for (var source : SpriteSourceList.load(resources, definition).list(resources)) {
            var contents = source.get(loader);
            if (contents != null) regions.put(contents.name(), new Sprite(sheet, contents));
        }
        var missing = regions.get(MissingTextureAtlasSprite.getLocation());
        return new SpriteLoader.Preparations(1, 1, 0, missing, regions, CompletableFuture.completedFuture(null));
    }

    @Override
    public TextureAtlasSprite get(SpriteId id) {
        var atlas = atlas(id.atlasLocation());
        return atlas.regions().getOrDefault(id.texture(), atlas.missing());
    }
}
