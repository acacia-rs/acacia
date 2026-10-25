package acacia.parity;

import java.util.HashMap;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.CompletableFuture;
import net.minecraft.client.renderer.texture.SpriteContents;
import net.minecraft.client.renderer.texture.SpriteLoader;
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import net.minecraft.client.renderer.texture.MissingTextureAtlasSprite;
import net.minecraft.client.renderer.texture.atlas.SpriteResourceLoader;
import net.minecraft.client.renderer.texture.atlas.SpriteSourceList;
import net.minecraft.resources.Identifier;

/**
 * An atlas's sprites as the client loads them (its atlases/*.json sources over the resources,
 * real images and metadata), each placed alone in a unit atlas: UVs come out sprite-local in [0,1].
 * Only the stitcher is skipped; it needs the client's options.
 */
public final class UnitAtlas {
    private UnitAtlas() {}

    private static final class Sprite extends TextureAtlasSprite {
        Sprite(Identifier atlas, SpriteContents contents) {
            super(atlas, contents, contents.width(), contents.height(), 0, 0, 0);
        }
    }

    /** `atlasInfo` as in AtlasIds (minecraft:blocks), `texture` as in TextureAtlas.LOCATION_*. */
    public static SpriteLoader.Preparations load(Identifier atlasInfo, Identifier texture) {
        var loader = SpriteResourceLoader.create(Set.of());
        Map<Identifier, TextureAtlasSprite> regions = new HashMap<>();
        for (var source : SpriteSourceList.load(Game.resources(), atlasInfo).list(Game.resources())) {
            var contents = source.get(loader);
            if (contents != null) regions.put(contents.name(), new Sprite(texture, contents));
        }
        var missing = regions.get(MissingTextureAtlasSprite.getLocation());
        return new SpriteLoader.Preparations(1, 1, 0, missing, regions, CompletableFuture.completedFuture(null));
    }
}
