package acacia.parity.entity_draws;

import acacia.parity.Client;
import acacia.parity.Headless;
import acacia.parity.Out;
import acacia.parity.Submits;
import com.mojang.blaze3d.vertex.PoseStack;
import java.lang.invoke.MethodHandles;
import java.lang.invoke.MethodType;
import java.lang.reflect.Field;
import net.minecraft.client.renderer.entity.GuardianRenderer;
import net.minecraft.client.renderer.entity.LivingEntityRenderer;
import net.minecraft.client.renderer.entity.state.GuardianRenderState;
import net.minecraft.world.entity.LivingEntity;
import net.minecraft.world.entity.monster.Guardian;
import java.lang.reflect.Modifier;
import java.nio.file.Path;
import java.util.*;
import net.minecraft.client.renderer.entity.EntityRenderer;
import net.minecraft.client.renderer.entity.state.EntityRenderState;
import net.minecraft.client.renderer.entity.state.LivingEntityRenderState;
import net.minecraft.client.renderer.state.level.CameraRenderState;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.resources.Identifier;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.EntitySpawnReason;
import net.minecraft.core.Holder;
import net.minecraft.core.registries.Registries;
import net.minecraft.resources.ResourceKey;
import net.minecraft.world.entity.Mob;
import net.minecraft.world.entity.animal.chicken.Chicken;
import net.minecraft.world.entity.animal.cow.Cow;
import net.minecraft.world.entity.animal.nautilus.ZombieNautilus;
import net.minecraft.world.entity.animal.pig.Pig;
import net.minecraft.world.entity.animal.wolf.Wolf;
import net.minecraft.world.level.Level;
import net.minecraft.world.entity.npc.villager.VillagerData;
import net.minecraft.world.item.DyeColor;

/**
 * Everything a kind's renderer submits for one entity (docs/parity.md, harness entity_draws),
 * args[0]/draws.jsonl: per kind and seed, the render state of a new entity with its own fields
 * set at random (booleans, enums, dyes, a pufferfish's puff; `fields`, as set), and each model
 * `EntityRenderer.submit` hands the collector, in order (as `Submits` records them).
 */
public final class EntityDrawsOracle {
    static final String[] KINDS = ("ender_dragon happy_ghast bat rabbit frog armadillo camel camel_husk sniffer breeze creaking warden copper_golem "
        + "nautilus zombie_nautilus tadpole zombie_villager axolotl armor_stand strider ravager shulker wither bee parrot fox horse donkey mule "
        + "guardian elder_guardian skeleton_horse zombie_horse cod salmon tropical_fish pufferfish phantom allay cat ocelot polar_bear panda goat "
        + "turtle hoglin zoglin wolf llama trader_llama squid glow_squid dolphin slime magma_cube piglin piglin_brute zombified_piglin enderman "
        + "witch vex vindicator evoker pillager silverfish endermite blaze ghast iron_golem snow_golem villager wandering_trader cow mooshroom pig "
        + "sheep chicken creeper spider cave_spider zombie husk drowned skeleton stray wither_skeleton bogged").split(" ");
    static final int SEEDS = 24;
    /**
     * Fields left as extracted: set by the kind (a trader llama, a brute), covered by entity_gear
     * (collars), out of a Bedrock server's reach (jeb_ sheep need the name), or already in the
     * texture the entity chose (an angry wolf's).
     */
    static final Set<String> FIXED = Set.of("isTraderLlama", "isBrute", "isJebSheep", "collarColor", "wolf.isAngry");

    public static void main(String[] args) throws Exception {
        Client.minecraft();
        var level = Client.overworld();
        Headless.set(Headless.get(level, "connection"), "scoreboard", new net.minecraft.world.scores.Scoreboard());
        var dispatcher = Client.entityRenderers();
        var layers = Submits.layerPrints();
        try (var out = new Out.Lines(Path.of(args[0]).resolve("draws.jsonl"))) {
            for (String kind : KINDS) {
                var type = BuiltInRegistries.ENTITY_TYPE.getValue(Identifier.withDefaultNamespace(kind));
                for (int seed = 0; seed < SEEDS; seed++) {
                    var record = new TreeMap<String, Object>();
                    record.put("case", "minecraft:" + kind + "|" + seed);
                    record.put("kind", "minecraft:" + kind);
                    try {
                        Entity entity = type.create(level, EntitySpawnReason.LOAD);
                        entity.setId(1_000_000);
                        if (entity instanceof Mob mob) mob.setBaby(seed % 3 == 1);
                        String climate = climate(entity, level, seed);
                        @SuppressWarnings("unchecked")
                        var renderer = (EntityRenderer<Entity, EntityRenderState>) dispatcher.getRenderer(entity);
                        var state = entity instanceof Guardian guardian ? guardian(renderer, guardian) : renderer.createRenderState(entity, 1.0F);
                        var fields = randomize(kind, state, new Random(seed * 131L + kind.hashCode()), seed);
                        if (climate != null) fields.put("climate", climate);
                        record.put("fields", fields);
                        var draws = new ArrayList<Map<String, Object>>();
                        renderer.submit(state, new PoseStack(), Submits.collector(draws, layers), new CameraRenderState());
                        record.put("draws", draws);
                    } catch (Throwable error) {
                        var cause = error.getCause() != null ? error.getCause() : error;
                        record.put("error", cause + " at " + Arrays.toString(Arrays.copyOf(cause.getStackTrace(), 4)));
                    }
                    out.write(record);
                }
            }
        }
    }

    /**
     * Sets the state's own booleans, enums, dyes and puff at random (seed 0 keeps them); returns
     * them as set, with the texture and villager data the entity gave.
     */
    private static Map<String, Object> randomize(String kind, EntityRenderState state, Random r, int seed) throws Exception {
        var set = new TreeMap<String, Object>();
        if (state instanceof LivingEntityRenderState living) set.put("isBaby", living.isBaby);
        set.put("ageInTicks", state.ageInTicks);
        for (Class<?> c = state.getClass(); c != LivingEntityRenderState.class && c != EntityRenderState.class; c = c.getSuperclass()) {
            for (Field f : c.getDeclaredFields()) {
                if (Modifier.isStatic(f.getModifiers())) continue;
                Class<?> t = f.getType();
                f.setAccessible(true);
                if (t == Identifier.class && f.get(state) instanceof Identifier id) set.put(f.getName(), id.getPath());
                if (t == VillagerData.class && f.get(state) instanceof VillagerData data) set.put(f.getName(), villager(data));
                if (FIXED.contains(f.getName()) || FIXED.contains(kind + "." + f.getName())) {
                    // As the entity set them (a tame wolf's collar, its anger).
                    var value = f.get(state);
                    if (value instanceof DyeColor d) set.put(f.getName(), d.getId());
                    else if (value instanceof Boolean b) set.put(f.getName(), b);
                    continue;
                }
                if (seed == 0) continue;
                Object value;
                if (t == boolean.class) value = r.nextBoolean();
                // A sheep's wool always has a colour; a shulker's may not.
                else if (t == DyeColor.class) value = r.nextInt(4) == 0 && !f.getName().equals("woolColor") ? null : DyeColor.byId(r.nextInt(16));
                else if (t.isEnum()) value = t.getEnumConstants()[r.nextInt(t.getEnumConstants().length)];
                else if (t == int.class && f.getName().equals("puffState")) value = r.nextInt(3);
                else if (t == int.class && f.getName().endsWith("Color")) {
                    // A tropical fish's colours: a dye's diffuse colour, recorded as the dye.
                    var dye = DyeColor.byId(r.nextInt(16));
                    Headless.set(state, f.getName(), dye.getTextureDiffuseColor());
                    set.put(f.getName(), dye.getId());
                    continue;
                } else continue;
                Headless.set(state, f.getName(), value);
                set.put(f.getName(), value instanceof DyeColor d ? (Object) d.getId() : value instanceof Enum<?> e ? e.name() : value);
            }
        }
        return set;
    }

    /**
     * GuardianRenderer.extractRenderState without its look target: that asks Minecraft's camera
     * entity, which the oracle lacks, and a Bedrock guardian with no target looks nowhere.
     */
    private static EntityRenderState guardian(EntityRenderer<Entity, EntityRenderState> renderer, Guardian entity) throws Throwable {
        var state = (GuardianRenderState) renderer.createRenderState();
        var extract = MethodHandles.privateLookupIn(GuardianRenderer.class, MethodHandles.lookup()).findSpecial(LivingEntityRenderer.class, "extractRenderState",
            MethodType.methodType(void.class, LivingEntity.class, LivingEntityRenderState.class, float.class), GuardianRenderer.class);
        extract.invoke(renderer, entity, state, 1.0F);
        state.spikesAnimation = entity.getSpikesAnimation(1.0F);
        state.tailAnimation = entity.getTailAnimation(1.0F);
        state.eyePosition = entity.getEyePosition(1.0F);
        return state;
    }

    static final String[] CLIMATES = {"temperate", "warm", "cold"};
    static final String[] WOLVES = {"pale", "ashen", "black", "chestnut", "rusty", "snowy", "spotted", "striped", "woods"};

    /**
     * Gives a cow, pig, chicken or zombie nautilus the climate variant `seed` picks (by seed / 3:
     * temperate, warm, cold; a nautilus has no cold), a wolf its variant (by seed / 3) and tameness
     * and anger (by seed % 4); returns the variant's name, null for other kinds.
     */
    @SuppressWarnings({"unchecked", "rawtypes"})
    private static String climate(Entity entity, Level level, int seed) {
        ResourceKey registry = switch (entity) {
            case Cow c -> Registries.COW_VARIANT;
            case Pig p -> Registries.PIG_VARIANT;
            case Chicken c -> Registries.CHICKEN_VARIANT;
            case ZombieNautilus n -> Registries.ZOMBIE_NAUTILUS_VARIANT;
            case Wolf w -> Registries.WOLF_VARIANT;
            default -> null;
        };
        if (registry == null) return null;
        String[] names = entity instanceof Wolf ? WOLVES : CLIMATES;
        String name = names[(seed / 3) % (entity instanceof ZombieNautilus ? 2 : names.length)];
        if (entity instanceof Wolf wolf) {
            wolf.setTame(seed % 4 == 1 || seed % 4 == 3, false);
            if (seed % 4 >= 2) wolf.setPersistentAngerEndTime(1_000_000L);
        }
        Holder<?> holder = (Holder<?>) level.registryAccess().lookupOrThrow(registry).get(Identifier.withDefaultNamespace(name)).orElseThrow();
        Headless.call(entity.getClass(), entity, "setVariant", new Class<?>[] {Holder.class}, holder);
        return name;
    }

    private static Map<String, Object> villager(VillagerData data) {
        var out = new TreeMap<String, Object>();
        out.put("type", data.type().unwrapKey().map(k -> k.identifier().getPath()).orElse(""));
        out.put("profession", data.profession().unwrapKey().map(k -> k.identifier().getPath()).orElse(""));
        out.put("level", data.level());
        return out;
    }
}
