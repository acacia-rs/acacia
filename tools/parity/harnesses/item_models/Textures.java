package acacia.parity.item_models;

import java.lang.reflect.Field;
import java.util.Map;
import net.minecraft.client.renderer.rendertype.RenderType;
import net.minecraft.resources.Identifier;

/** The texture a render type samples: its RenderSetup's Sampler0. */
final class Textures {
    private static Field state;
    private static Field textures;

    private Textures() {}

    @SuppressWarnings("unchecked")
    static synchronized String of(RenderType type) {
        try {
            if (state == null) {
                state = RenderType.class.getDeclaredField("state");
                state.setAccessible(true);
                textures = state.getType().getDeclaredField("textures");
                textures.setAccessible(true);
            }
            var binding = ((Map<String, ?>) textures.get(state.get(type))).get("Sampler0");
            var location = binding.getClass().getDeclaredMethod("location");
            location.setAccessible(true);
            return ((Identifier) location.invoke(binding)).toString();
        } catch (ReflectiveOperationException error) {
            throw new IllegalStateException(error);
        }
    }
}
