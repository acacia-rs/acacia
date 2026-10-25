package acacia.parity.glint;

import acacia.parity.Client;
import acacia.parity.Headless;
import acacia.parity.Out;
import java.nio.file.Path;
import net.minecraft.client.renderer.GameRenderer;
import net.minecraft.client.renderer.rendertype.TextureTransform;
import net.minecraft.client.renderer.state.GameRenderState;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.util.Util;
import net.minecraft.world.item.ItemStack;
import org.joml.Matrix4f;

/**
 * The enchantment glint (docs/parity.md, harness glint), args[0]/glint.jsonl:
 * `texturing|<name>|<millis>|<speed>`: the texture matrix a TextureTransform (GLINT_TEXTURING,
 * ENTITY_GLINT_TEXTURING, ARMOR_ENTITY_GLINT_TEXTURING) gives at that clock (Util's time source)
 * and glintSpeed option, column-major; `foil|<item>`: whether the item's default stack shimmers
 * (ItemStack.hasFoil).
 */
public final class GlintOracle {
    public static void main(String[] args) throws Exception {
        var minecraft = Client.minecraft();
        acacia.parity.Data.get();
        var renderer = Headless.allocate(GameRenderer.class);
        Headless.set(renderer, "gameRenderState", new GameRenderState());
        Headless.set(minecraft, "gameRenderer", renderer);
        var options = renderer.gameRenderState().optionsRenderState;
        var random = new java.util.Random(26_3);
        try (var out = new Out.Lines(Path.of(args[0]).resolve("glint.jsonl"))) {
            for (String name : new String[] {"GLINT_TEXTURING", "ENTITY_GLINT_TEXTURING", "ARMOR_ENTITY_GLINT_TEXTURING"}) {
                var transform = (TextureTransform) TextureTransform.class.getField(name).get(null);
                for (int i = 0; i < 400; i++) {
                    long millis = i < 4 ? new long[] {0, 1, 27_499, 110_000 / 4}[i] : (long) (random.nextDouble() * 4e9);
                    double speed = i % 3 == 0 ? 0.5 : random.nextDouble();
                    options.glintSpeed = speed;
                    Util.setTimeSource(() -> millis * 1_000_000L);
                    Matrix4f m = transform.createMatrix();
                    out.write(Out.map("case", "texturing|" + name + "|" + millis + "|" + speed, "transform", name, "millis", millis, "speed", speed,
                        "matrix", m.get(new float[16])));
                }
            }
            for (var item : BuiltInRegistries.ITEM) {
                var id = BuiltInRegistries.ITEM.getKey(item).toString();
                out.write(Out.map("case", "foil|" + id, "item", id, "foil", new ItemStack(item).hasFoil()));
            }
        }
    }
}
