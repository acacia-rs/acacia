"""The look's Java entity and item models for the entity and item harnesses: `lookbake java-exported` on the pinned client jar."""
import os

from paritylib.home import GAME, HOME, VERSION


def look_dir(h):
    """Exports the models into the harness's run dir; returns the look dir."""
    java = HOME / "lookbake-java"
    jar = java / f"client-{VERSION}.jar"
    if not jar.exists():
        java.mkdir(parents=True, exist_ok=True)
        jar.symlink_to(GAME / "client.jar")
    os.environ.setdefault("JAVA_HOME", str(HOME / "jdk"))
    out = h.out / "look"
    h.cargo("run", "--release", "-q", "-p", "lookbake", "--", "java-exported", java, out)
    return out


def look(h):
    """The exported look's entity_models.json."""
    return look_dir(h) / "java" / "entity_models.json"
