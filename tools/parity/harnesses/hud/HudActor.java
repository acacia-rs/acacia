package acacia.parity.gui;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import java.lang.reflect.Proxy;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.core.Holder;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.tags.FluidTags;
import net.minecraft.tags.TagKey;
import net.minecraft.world.effect.MobEffect;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.HumanoidArm;
import net.minecraft.world.entity.ai.attributes.Attribute;
import net.minecraft.world.entity.ai.attributes.Attributes;
import net.minecraft.world.food.FoodData;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.material.Fluid;

/**
 * The HUD oracle's player: its HUD-facing getters answer from the case. Adapted from Horizon's
 * VanillaHudOracle (MIT, (c) Ludovic Marie).
 */
public final class HudActor extends LocalPlayer {
    static JsonObject c;
    Level level;
    Entity vehicle;
    FoodData food;
    ItemStack offhand = ItemStack.EMPTY;
    List<net.minecraft.world.effect.MobEffectInstance> active = new ArrayList<>();

    private HudActor() { super(null, null, null, null, null, null, false, null, null); }

    static float f(String k, float d) { return c.has(k) && !c.get(k).isJsonNull() ? c.get(k).getAsFloat() : d; }
    static int i(String k, int d) { return c.has(k) && !c.get(k).isJsonNull() ? c.get(k).getAsInt() : d; }
    static boolean b(String k) { return c.has(k) && !c.get(k).isJsonNull() && c.get(k).getAsBoolean(); }

    @Override public float getHealth() { return f("health", 20); }
    @Override public double getAttributeValue(Holder<Attribute> a) { return a == Attributes.MAX_HEALTH ? f("maxHealth", 20) : 0.0; }
    @Override public float getAbsorptionAmount() { return f("absorption", 0); }
    @Override public int getArmorValue() { return i("armor", 0); }
    @Override public FoodData getFoodData() { return food; }
    @Override public boolean hasEffect(Holder<MobEffect> effect) {
        if (c.has("effects")) for (JsonElement e : c.getAsJsonArray("effects")) if (effect.is(Identifier.parse(e.getAsString()))) return true;
        return false;
    }
    @Override public java.util.Collection<net.minecraft.world.effect.MobEffectInstance> getActiveEffects() { return active; }
    @Override public boolean isFullyFrozen() { return b("frozen"); }
    @Override public int getAirSupply() { return i("air", 300); }
    @Override public int getMaxAirSupply() { return i("maxAir", 300); }
    @Override public boolean isEyeInFluid(TagKey<Fluid> tag) { return tag == FluidTags.WATER && b("underWater"); }
    @Override public Entity getVehicle() { return vehicle; }
    @Override public ItemStack getOffhandItem() { return offhand; }
    @Override public HumanoidArm getMainArm() { return b("rightHanded") ? HumanoidArm.RIGHT : HumanoidArm.LEFT; }
    @Override public float getAttackStrengthScale(float a) { return f("attackScale", 1); }
    @Override public float getCurrentItemAttackStrengthDelay() { return f("attackDelay", 5); }
    @Override public ItemStack getActiveItem() { return ItemStack.EMPTY; }
    @Override public Level level() { return level; }
    @Override public boolean isSpectator() { return false; }
    @Override public net.minecraft.world.entity.PlayerRideableJumping jumpableVehicle() {
        return (net.minecraft.world.entity.PlayerRideableJumping) Proxy.newProxyInstance(HudActor.class.getClassLoader(),
            new Class<?>[] {net.minecraft.world.entity.PlayerRideableJumping.class},
            (proxy, method, a) -> method.getName().equals("getJumpCooldown") ? i("cooldown", 0)
                : method.getReturnType() == boolean.class ? false : method.getReturnType() == float.class ? 0.0F : null);
    }
    @Override public float getJumpRidingScale() { return f("jumpScale", 0); }
    @Override public ItemStack getItemBySlot(net.minecraft.world.entity.EquipmentSlot slot) {
        return slot == net.minecraft.world.entity.EquipmentSlot.HEAD && c.has("head") && !c.get("head").isJsonNull()
            ? new ItemStack(BuiltInRegistries.ITEM.getValue(Identifier.withDefaultNamespace(c.get("head").getAsString()))) : ItemStack.EMPTY;
    }
    @Override public int getTicksFrozen() { return i("ticksFrozen", 0); }
    @Override public boolean isScoping() { return b("scoping"); }
    @Override public float getEffectBlendFactor(Holder<MobEffect> effect, float partial) {
        return effect.is(net.minecraft.world.effect.MobEffects.NAUSEA) ? f("nausea", 0) : 0.0F;
    }
    @Override public int getSleepTimer() { return i("sleepTimer", 0); }
    @Override public void playSound(net.minecraft.sounds.SoundEvent sound, float volume, float pitch) { }

    /** A ridden mount answering its health from the case. */
    public static final class Mount extends net.minecraft.world.entity.animal.equine.Horse {
        private Mount() { super(null, null); }
        @Override public float getHealth() { return f("vehicleHealth", 0); }
        @Override public double getAttributeValue(Holder<Attribute> a) { return a == Attributes.MAX_HEALTH ? f("vehicleMaxHealth", 0) : 0.0; }
        @Override public boolean showVehicleHealth() { return true; }
    }

    /** `Minecraft.crosshairPickEntity`: a living entity, alive as the case says. */
    public static final class Target extends net.minecraft.world.entity.animal.equine.Horse {
        private Target() { super(null, null); }
        @Override public boolean isAlive() { return b("target"); }
    }
}
