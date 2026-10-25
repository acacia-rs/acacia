"""The Java look lookbake bakes from the pinned client jar, for harnesses whose probe reads it."""
from .home import GAME, HOME, VERSION


def java_dir():
    """lookbake's Java dir, its jar the oracle's (no download)."""
    java = HOME / "lookbake-java"
    jar = java / f"client-{VERSION}.jar"
    if not jar.exists():
        java.mkdir(parents=True, exist_ok=True)
        jar.symlink_to(GAME / "client.jar")
    return java


def environment_json(h):
    """`lookbake java-environment`: the look's environment.json, written under the run's dir."""
    out = h.out / "look"
    h.cargo("run", "--release", "-q", "-p", "lookbake", "--", "java-environment", java_dir(), out)
    return out / "environment.json"
