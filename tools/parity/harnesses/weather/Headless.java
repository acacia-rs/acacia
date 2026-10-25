package acacia.parity.weather;

import java.lang.reflect.Field;
import net.minecraft.client.Camera;
import net.minecraft.client.Minecraft;
import net.minecraft.client.OptionInstance;
import net.minecraft.client.Options;
import net.minecraft.client.renderer.GameRenderer;
import net.minecraft.world.phys.Vec3;

/**
 * Objects built without their constructors (no window, no GPU), and a `Minecraft.getInstance()`
 * holding only what the weather code reads: the camera, Weather Radius and Particles options.
 */
final class Headless {
    private static final Object UNSAFE;

    static {
        try {
            Field field = Class.forName("sun.misc.Unsafe").getDeclaredField("theUnsafe");
            field.setAccessible(true);
            UNSAFE = field.get(null);
        } catch (ReflectiveOperationException e) {
            throw new ExceptionInInitializerError(e);
        }
    }

    private Headless() {}

    static <T> T allocate(Class<T> type) throws Exception {
        return type.cast(UNSAFE.getClass().getMethod("allocateInstance", Class.class).invoke(UNSAFE, type));
    }

    /** Sets `name`, declared on `owner` or one of its superclasses, final or not. */
    static void set(Object owner, String name, Object value) throws Exception {
        field(owner instanceof Class<?> c ? c : owner.getClass(), name).set(owner instanceof Class<?> ? null : owner, value);
    }

    static Object get(Object owner, String name) throws Exception {
        return field(owner.getClass(), name).get(owner);
    }

    private static Field field(Class<?> type, String name) throws NoSuchFieldException {
        for (Class<?> c = type; c != null; c = c.getSuperclass()) {
            try {
                Field field = c.getDeclaredField(name);
                field.setAccessible(true);
                return field;
            } catch (NoSuchFieldException ignored) {
                // a superclass's
            }
        }
        throw new NoSuchFieldException(name);
    }

    static final class Client {
        final Minecraft minecraft;
        private final Camera camera;
        private final OptionInstance<?> weatherRadius;
        private final OptionInstance<?> particles;

        Client() throws Exception {
            minecraft = allocate(Minecraft.class);
            Options options = allocate(Options.class);
            weatherRadius = allocate(OptionInstance.class);
            particles = allocate(OptionInstance.class);
            set(options, "weatherRadius", weatherRadius);
            set(options, "particles", particles);
            GameRenderer renderer = allocate(GameRenderer.class);
            camera = allocate(Camera.class);
            set(renderer, "mainCamera", camera);
            set(minecraft, "options", options);
            set(minecraft, "gameRenderer", renderer);
            set(Minecraft.class, "instance", minecraft);
        }

        void camera(Vec3 position) throws Exception {
            set(camera, "position", position);
        }

        void weatherRadius(int radius) throws Exception {
            set(weatherRadius, "value", radius);
        }

        void particles(Object status) throws Exception {
            set(particles, "value", status);
        }
    }
}
