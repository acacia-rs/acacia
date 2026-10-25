package acacia.parity.gui;

import com.google.gson.JsonArray;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import java.util.HashSet;
import java.util.Set;
import net.minecraft.core.Holder;
import net.minecraft.core.component.DataComponentMap;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.RegistryDataLoader;
import net.minecraft.resources.ResourceKey;
import net.minecraft.tags.EnchantmentTags;
import net.minecraft.tags.TagKey;
import net.minecraft.world.effect.MobEffectInstance;
import net.minecraft.world.entity.ai.attributes.Attributes;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.enchantment.Enchantment;
import net.minecraft.world.item.enchantment.ItemEnchantments;

/**
 * The registry facts a case's stacks refer to, for the probe (`game` in its input): every string in
 * the resolved stacks naming an entry of a registry tooltips read is looked up, and the entry's
 * data (data-pack entries in their own codec's JSON; built-in ones as the few fields read) written
 * under the registry's name, following references from what was written. Also the
 * `tooltip_order` tag, each stack's enchantment iteration order (a hash order) and the player's
 * base attack values. Nothing is written that the case does not refer to.
 */
final class TooltipFacts {
    private static final Set<String> DATA = Set.of("enchantment", "instrument", "jukebox_song", "trim_pattern", "trim_material", "banner_pattern", "painting_variant");

    private final JsonObject game;
    private final Set<String> seen = new HashSet<>();

    private TooltipFacts(JsonObject game) {
        this.game = game;
    }

    static void collect(ScreenKit kit) throws Exception {
        var facts = new TooltipFacts(kit.game);
        for (var e : kit.items.entrySet()) facts.scan(e.getValue());
        var order = new JsonArray();
        GuiGame.lookup.lookupOrThrow(Registries.ENCHANTMENT).get(EnchantmentTags.TOOLTIP_ORDER)
            .ifPresent(set -> set.forEach(h -> order.add(h.getRegisteredName())));
        kit.game.add("tooltipOrder", order);
        var orders = new JsonObject();
        for (var key : kit.capture.keys.entrySet()) {
            var o = new JsonObject();
            o.add("enchantments", names(key.getKey().getOrDefault(net.minecraft.core.component.DataComponents.ENCHANTMENTS, ItemEnchantments.EMPTY)));
            o.add("stored_enchantments", names(key.getKey().getOrDefault(net.minecraft.core.component.DataComponents.STORED_ENCHANTMENTS, ItemEnchantments.EMPTY)));
            orders.add(Integer.toString(key.getValue()), o);
        }
        kit.game.add("enchantmentOrder", orders);
        var player = new JsonObject();
        player.addProperty("attack_damage", kit.player.getAttributeBaseValue(Attributes.ATTACK_DAMAGE));
        player.addProperty("attack_speed", kit.player.getAttributeBaseValue(Attributes.ATTACK_SPEED));
        kit.game.add("player", player);
    }

    static JsonArray names(ItemEnchantments enchantments) {
        var out = new JsonArray();
        enchantments.keySet().forEach(h -> out.add(h.getRegisteredName()));
        return out;
    }

    void scan(JsonElement json) {
        if (json.isJsonObject()) {
            for (var e : json.getAsJsonObject().entrySet()) {
                name(e.getKey());
                scan(e.getValue());
            }
        } else if (json.isJsonArray()) {
            json.getAsJsonArray().forEach(this::scan);
        } else if (json.isJsonPrimitive() && json.getAsJsonPrimitive().isString()) {
            name(json.getAsString());
        }
    }

    private JsonObject table(String name) {
        if (!game.has(name)) game.add(name, new JsonObject());
        return game.getAsJsonObject(name);
    }

    private void put(String table, String id, JsonObject fact) {
        table(table).add(id, fact);
        scan(fact);
    }

    void name(String s) {
        if (!seen.add(s)) return;
        if (s.startsWith("#")) {
            var id = Identifier.tryParse(s.substring(1));
            if (id == null) return;
            GuiGame.lookup.lookupOrThrow(Registries.BLOCK).get(TagKey.create(Registries.BLOCK, id)).ifPresent(set -> {
                var blocks = new JsonArray();
                set.forEach(h -> {
                    blocks.add(h.getRegisteredName());
                    block(h.getRegisteredName(), h.value());
                });
                table("blockTags").add(s, blocks);
            });
            return;
        }
        var id = Identifier.tryParse(s);
        if (id == null) return;
        String key = id.toString();
        for (RegistryDataLoader.RegistryData<?> data : RegistryDataLoader.WORLD_REGISTRIES) {
            if (DATA.contains(data.key().identifier().getPath())) dataEntry(data, id, key);
        }
        BuiltInRegistries.ITEM.getOptional(id).ifPresent(item -> {
            var o = new JsonObject();
            o.add("components", DataComponentMap.CODEC.encodeStart(ScreenKit.ops(), item.components()).getOrThrow());
            o.addProperty("enabled", item.isEnabled(net.minecraft.world.flag.FeatureFlags.DEFAULT_FLAGS));
            o.addProperty("blockItem", item instanceof net.minecraft.world.item.BlockItem);
            // Not followed: an item's defaults name much and no tooltip reads it further.
            table("item").add(key, o);
        });
        BuiltInRegistries.BLOCK.getOptional(id).ifPresent(block -> block(key, block));
        BuiltInRegistries.ENTITY_TYPE.getOptional(id).ifPresent(type -> {
            var o = new JsonObject();
            o.addProperty("description", type.getDescriptionId());
            o.addProperty("peaceful", type.isAllowedInPeaceful());
            o.addProperty("op", type.onlyOpCanSetNbt());
            table("entity_type").add(key, o);
        });
        BuiltInRegistries.BLOCK_ENTITY_TYPE.getOptional(id).ifPresent(type -> {
            var o = new JsonObject();
            o.addProperty("op", type.onlyOpCanSetNbt());
            table("block_entity_type").add(key, o);
        });
        BuiltInRegistries.ATTRIBUTE.getOptional(id).ifPresent(attribute -> {
            var o = new JsonObject();
            o.addProperty("description", attribute.getDescriptionId());
            o.addProperty("positive", attribute.getStyle(true).name().toLowerCase(java.util.Locale.ROOT));
            o.addProperty("negative", attribute.getStyle(false).name().toLowerCase(java.util.Locale.ROOT));
            table("attribute").add(key, o);
        });
        BuiltInRegistries.MOB_EFFECT.get(id).ifPresent(effect -> {
            var o = new JsonObject();
            o.addProperty("description", effect.value().getDescriptionId());
            o.addProperty("category", effect.value().getCategory().getTooltipFormatting().name().toLowerCase(java.util.Locale.ROOT));
            var modifiers = new JsonArray();
            effect.value().createModifiers(0, (attribute, modifier) -> {
                var m = new JsonObject();
                m.addProperty("attribute", attribute.getRegisteredName());
                m.addProperty("amount", modifier.amount());
                m.addProperty("operation", modifier.operation().getSerializedName());
                modifiers.add(m);
            });
            o.add("modifiers", modifiers);
            put("mob_effect", key, o);
        });
        BuiltInRegistries.POTION.getOptional(id).ifPresent(potion -> {
            var o = new JsonObject();
            o.addProperty("name", potion.name());
            var effects = new JsonArray();
            for (MobEffectInstance effect : potion.getEffects()) effects.add(MobEffectInstance.CODEC.encodeStart(ScreenKit.ops(), effect).getOrThrow());
            o.add("effects", effects);
            put("potion", key, o);
        });
    }

    private void block(String key, net.minecraft.world.level.block.Block block) {
        var o = new JsonObject();
        o.addProperty("description", block.getDescriptionId());
        table("block").add(key, o);
    }

    private <T> void dataEntry(RegistryDataLoader.RegistryData<T> data, Identifier id, String key) {
        var registry = GuiGame.lookup.lookupOrThrow(data.key());
        registry.get(ResourceKey.create(data.key(), id)).ifPresent(holder -> {
            var json = data.elementCodec().encodeStart(ScreenKit.ops(), holder.value()).getOrThrow().getAsJsonObject();
            if (holder.value() instanceof Enchantment) {
                @SuppressWarnings("unchecked")
                var h = (Holder<Enchantment>) (Holder<?>) holder;
                json.addProperty("curse", h.is(EnchantmentTags.CURSE));
            }
            put(data.key().identifier().getPath(), key, json);
        });
    }
}
