package acacia.parity.particles;

import acacia.parity.Atlases;
import acacia.parity.Game;
import acacia.parity.Shell;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import net.minecraft.client.particle.ParticleDescription;
import net.minecraft.client.particle.ParticleResources;
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import net.minecraft.client.resources.sounds.SoundInstance;
import net.minecraft.client.sounds.SoundEngine;
import net.minecraft.client.sounds.SoundManager;
import net.minecraft.data.AtlasIds;
import net.minecraft.resources.Identifier;
import net.minecraft.util.GsonHelper;

/** ParticleResources as its reload leaves it, headless: providers registered, sprite sets bound to the particle atlas. */
final class Sprites {
    private Sprites() {}

    static ParticleResources resources() throws Exception {
        var resources = new ParticleResources();
        var atlas = Atlases.prepared(AtlasIds.PARTICLES);
        Map<Identifier, Object> sets = Shell.get(resources, "spriteSets");
        for (var entry : sets.entrySet()) {
            var id = entry.getKey();
            var file = Identifier.fromNamespaceAndPath(id.getNamespace(), "particles/" + id.getPath() + ".json");
            var resource = Game.resources().getResource(file).orElseThrow(() -> new IllegalStateException("no " + file));
            List<Identifier> textures;
            try (var reader = resource.openAsReader()) {
                textures = ParticleDescription.fromJson(GsonHelper.parse(reader)).getTextures();
            }
            var sprites = new ArrayList<TextureAtlasSprite>();
            for (var texture : textures) {
                var sprite = atlas.getSprite(texture);
                sprites.add(sprite == null ? atlas.missing() : sprite);
            }
            if (sprites.isEmpty()) sprites.add(atlas.missing());
            Shell.call(entry.getValue().getClass(), entry.getValue(), "rebind", new Class<?>[]{List.class}, sprites);
        }
        return resources;
    }

    /** Takes ClientLevel's local sounds (which still draw the level random for their seeds) and plays nothing. */
    static final class Silent extends SoundManager {
        private Silent() {
            super(null);
        }

        @Override public SoundEngine.PlayResult play(SoundInstance instance) { return SoundEngine.PlayResult.NOT_STARTED; }
        @Override public void playDelayed(SoundInstance instance, int delay) {}
    }

    static SoundManager silent() {
        return Shell.alloc(Silent.class);
    }
}
