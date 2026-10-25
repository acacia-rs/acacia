package acacia.parity;

import com.mojang.renderpearl.api.textures.GpuTexture;
import com.mojang.renderpearl.api.textures.GpuTextureView;
import java.util.HashMap;
import java.util.Map;
import net.minecraft.client.renderer.texture.AbstractTexture;
import net.minecraft.client.renderer.texture.TextureManager;
import net.minecraft.resources.Identifier;

/**
 * Textures without a GPU: every texture the client would bind is a NamedView carrying its id, so
 * draw records (TextureSetup) name what they sample. Atlases are Atlases' shells.
 */
public final class Textures {
    private Textures() {}

    /** Stands for a texture's view; only its id is real. */
    public record NamedView(Identifier id) implements GpuTextureView {
        @Override public boolean isClosed() { return false; }
        @Override public GpuTexture texture() { return null; }
        @Override public int baseMipLevel() { return 0; }
        @Override public int mipLevels() { return 1; }
        @Override public int getWidth(int mipLevel) { return 0; }
        @Override public int getHeight(int mipLevel) { return 0; }
        @Override public void close() {}
    }

    /** A plain texture: a shell whose view names it. Allocated, never constructed. */
    static final class Named extends AbstractTexture {
        private Named() {}
    }

    /** Answers getTexture with the atlas shells or a Named texture. Allocated, never constructed. */
    static final class Manager extends TextureManager {
        Map<Identifier, AbstractTexture> textures;

        private Manager() {
            super(null);
        }

        @Override
        public AbstractTexture getTexture(Identifier location) {
            return textures.computeIfAbsent(location, id -> named(id));
        }

        @Override
        public void register(Identifier location, AbstractTexture texture) {
            textures.put(location, texture);
        }
    }

    private static final Map<Identifier, String> SPRITE_TEXTURES = new java.util.concurrent.ConcurrentHashMap<>();

    /** A stand-in texture id for one sprite of an atlas (UnitAtlas nameSprites); describe names it `atlas#sprite`. */
    public static Identifier spriteTexture(Identifier atlasTexture, Identifier sprite) {
        var id = atlasTexture.withSuffix("/" + sprite.getNamespace() + "/" + sprite.getPath());
        SPRITE_TEXTURES.put(id, atlasTexture + "#" + sprite);
        return id;
    }

    /** A texture id as draw records name it: `minecraft:textures/atlas/gui.png#minecraft:hud/heart/full` for a named sprite. */
    public static String describe(Identifier texture) {
        return SPRITE_TEXTURES.getOrDefault(texture, texture.toString());
    }

    private static AbstractTexture named(Identifier id) {
        var texture = Shell.alloc(Named.class);
        Shell.set(texture, "textureView", new NamedView(id));
        return texture;
    }

    /** A TextureManager holding `atlases` (by texture id) and naming everything else on demand. */
    public static TextureManager manager(Map<Identifier, ? extends AbstractTexture> atlases) {
        var manager = Shell.alloc(Manager.class);
        manager.textures = new HashMap<>(atlases);
        return manager;
    }
}
