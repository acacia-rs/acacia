"""The acacia-anvil acceptance test (README.md "Oracle"): Java BlueMap's render of a Paper world against
bluemap-rs's render of what an acacia bot, joined through Geyser, saw of it.

Usage: py -3 crates/acacia-anvil/tools/oracle.py [all|server|crawl|trim|golden|candidate|diff] [--radius 8]
  [--dims overworld,the_nether,the_end|all] [--memory BYTES]

Lab: <repo>/.testserver/anvil-lab (git-ignored): Paper 26.3 + Geyser-Spigot (offline, Bedrock :19150),
fixed seed. Java, BlueMap 5.29 and the MC client jar are read from the bluemap-rs checkout (BLUEMAP_RS).
"""
import argparse
import json
import os
import shutil
import subprocess
import sys
import threading
import time
from pathlib import Path

import scene
from regions import read_region, write_region

REPO = Path(__file__).resolve().parents[3]
REPORT = Path(__file__).resolve().parent / ".cache/java-26.3-blocks.json"
LAB = REPO / ".testserver" / "anvil-lab"
SERVER = LAB / "server"
BMRS = Path(os.environ.get("BLUEMAP_RS", "C:/Users/hexay/bluemap-rs"))
JAVA = BMRS / "work/downloads/jdk25/bin" / ("java.exe" if os.name == "nt" else "java")
BLUEMAP_JAR = BMRS / "work/downloads/bluemap-5.29-cli.jar"
CLIENT_JAR = BMRS / "work/bluemap/vanilla/data/minecraft-client-26.3.jar"
# bluemap-rs binaries built from that checkout into the lab (its own target dir stays untouched)
BM_TARGET = LAB / "bm-target"
# per dimension: map id, BlueMap's own map config for it (from bluemap-rs's fixtures), where the bot goes
DIMS = {
    "overworld": ("lab", BMRS / "work/bluemap/vanilla/config/maps/vanilla.conf", None),
    "the_nether": ("lab_nether", BMRS / "work/bluemap/nether/config/maps/nether.conf", "the_nether 0 64 0"),
    "the_end": ("lab_end", BMRS / "work/bluemap/end/config/maps/end.conf", "the_end 0 70 0"),
}
OVERWORLD = "dimensions/minecraft/overworld/region"


def region_rel(dim):
    return f"dimensions/minecraft/{dim}/region"

SEED = "acacia-anvil"
PROPERTIES = {
    "online-mode": "false", "server-port": "25570", "level-seed": SEED, "view-distance": "12",
    "simulation-distance": "4", "spawn-protection": "0", "difficulty": "peaceful", "gamemode": "creative",
    "motd": "acacia-anvil lab", "spawn-monsters": "false", "white-list": "false", "enforce-whitelist": "false",
}
# Java's offline UUID for "Anvil" (as Geyser offline auth logs it in), opped so ANVIL_TP works
OPS = [{"uuid": "0cc89651-436f-38c2-a44d-d70fbab49a5c", "name": "Anvil", "level": 4, "bypassesPlayerLimit": False}]
GEYSER_PORT = 19150


def run(cmd, **kw):
    print("+", " ".join(str(c) for c in cmd), flush=True)
    subprocess.run([str(c) for c in cmd], check=True, **kw)


def write_server_config():
    SERVER.mkdir(parents=True, exist_ok=True)
    (SERVER / "eula.txt").write_text("eula=true\n")
    (SERVER / "server.properties").write_text("".join(f"{k}={v}\n" for k, v in PROPERTIES.items()))
    (SERVER / "ops.json").write_text(json.dumps(OPS))


def geyser_config():
    """Geyser writes its config on first start; point it at our ports and offline auth."""
    cfg = SERVER / "plugins/Geyser-Spigot/config.yml"
    if not cfg.exists():
        return False
    lines = []
    section = None
    for line in cfg.read_text().splitlines():
        stripped = line.strip()
        if line and not line[0].isspace() and stripped.endswith(":"):
            section = stripped[:-1]
        if section == "bedrock" and stripped.startswith("port:"):
            line = line[: line.index("port:")] + f"port: {GEYSER_PORT}"
        if stripped.startswith("auth-type:"):
            line = line[: line.index("auth-type:")] + "auth-type: offline"
        # the bot joins offline: no Xbox chain to validate
        if stripped.startswith("validate-bedrock-login:"):
            line = line[: line.index("validate-bedrock-login:")] + "validate-bedrock-login: false"
        lines.append(line)
    cfg.write_text("\n".join(lines) + "\n")
    return True


class Server:
    def __init__(self):
        self.lines = []
        self.proc = subprocess.Popen([str(JAVA), "-Xmx2G", "-jar", "paper.jar", "--nogui"], cwd=SERVER,
                                     stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                     text=True, encoding="utf-8", errors="replace")
        threading.Thread(target=self._read, daemon=True).start()

    def _read(self):
        with open(LAB / "server.log", "a", encoding="utf-8") as log:
            for line in self.proc.stdout:
                self.lines.append(line)
                log.write(line)
                log.flush()

    def wait_for(self, text, timeout=300):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            if any(text in l for l in self.lines):
                return
            if self.proc.poll() is not None:
                raise SystemExit(f"server exited waiting for {text!r}")
            time.sleep(0.5)
        raise SystemExit(f"timed out waiting for {text!r}")

    def command(self, cmd):
        self.proc.stdin.write(cmd + "\n")
        self.proc.stdin.flush()

    def stop(self):
        self.command("save-all flush")
        self.command("stop")
        self.proc.wait(timeout=120)


def start_server():
    write_server_config()
    # the bot rejoins where it left otherwise; it should spawn at the scene
    shutil.rmtree(SERVER / "world/players", ignore_errors=True)
    first = not geyser_config()
    server = Server()
    server.wait_for("Done (")
    if first:
        server.stop()
        if not geyser_config():
            raise SystemExit("Geyser wrote no config")
        server = Server()
        server.wait_for("Done (")
    return server


def crawl(args):
    cand = LAB / "candidate/world"
    shutil.rmtree(cand, ignore_errors=True)
    env = {**os.environ, **({"ANVIL_MEMORY": args.memory} if args.memory is not None else {})}
    stops = [DIMS[d][2] for d in args.dims if DIMS[d][2]]
    if stops:
        env["ANVIL_TP"] = ";".join(stops)
    run(["cargo", "run", "--release", "-p", "acacia-anvil", "--example", "crawl", "--",
         f"127.0.0.1:{GEYSER_PORT}", cand, "Anvil", args.radius, args.quiet, args.limit], cwd=REPO, env=env)


def crawled_dims():
    return [d for d in DIMS if (LAB / "candidate/world" / region_rel(d)).is_dir()]


def trim():
    """The server's world cut to the chunks the bot saw, so both renders cover one area."""
    shutil.rmtree(LAB / "golden/world", ignore_errors=True)
    for dim in crawled_dims():
        out, src_dir, kept = LAB / "golden/world" / region_rel(dim), SERVER / "world" / region_rel(dim), 0
        for f in sorted((LAB / "candidate/world" / region_rel(dim)).glob("r.*.mca")):
            keep = read_region(f).keys()
            src = src_dir / f.name
            chunks = {i: c for i, c in read_region(src).items() if i in keep} if src.exists() else {}
            missing = len(keep) - len(chunks)
            if missing:
                print(f"{dim} {f.name}: {missing} chunks the server never saved")
            write_region(out / f.name, chunks)
            kept += len(chunks)
        print(f"golden {dim}: {kept} chunks")


def bluemap_dir(name, world, dims=("overworld",)):
    base = LAB / name
    cfg = base / "config"
    if not (cfg / "core.conf").exists():
        base.mkdir(parents=True, exist_ok=True)
        subprocess.call([str(JAVA), "-jar", str(BLUEMAP_JAR), "-c", "config", "-v", "26.3"], cwd=base)
        text = (cfg / "core.conf").read_text().replace("accept-download: false", "accept-download: true")
        (cfg / "core.conf").write_text(text.replace("metrics: true", "metrics: false"))
    (base / "data").mkdir(exist_ok=True)
    if not (base / "data" / CLIENT_JAR.name).exists():
        shutil.copy(CLIENT_JAR, base / "data" / CLIENT_JAR.name)
    for f in (cfg / "maps").glob("*.conf"):
        f.unlink()
    for dim in dims:
        map_id, template, _ = DIMS[dim]
        conf = [f'world: {json.dumps(world.as_posix())}' if l.startswith("world:") else
                f'name: "{map_id}"' if l.startswith("name:") else l for l in template.read_text().splitlines()]
        (cfg / f"maps/{map_id}.conf").write_text("\n".join(conf) + "\n")
    shutil.rmtree(base / "web/maps", ignore_errors=True)
    return base


def build_bluemap_rs():
    run(["cargo", "build", "--profile", "fast", "--locked", "-j", "4", "-p", "bm-cli", "-p", "bm-golden"],
        cwd=BMRS, env={**os.environ, "CARGO_TARGET_DIR": str(BM_TARGET)})
    return BM_TARGET / "fast"


def golden():
    base = bluemap_dir("bm-java", LAB / "golden/world", crawled_dims())
    run([JAVA, "-jar", BLUEMAP_JAR, "-c", "config", "-v", "26.3", "-r", "-f"], cwd=base)


def candidate():
    exe = build_bluemap_rs() / ("bluemap.exe" if os.name == "nt" else "bluemap")
    base = bluemap_dir("bm-rs", LAB / "candidate/world", crawled_dims())
    run([exe, "-c", "config", "-v", "26.3", "-r", "-f"], cwd=base)


def diff(args):
    exe = build_bluemap_rs() / ("bm-golden.exe" if os.name == "nt" else "bm-golden")
    for dim in crawled_dims():
        map_id = DIMS[dim][0]
        print(f"== {dim} (map {map_id})", flush=True)
        run([exe, "diff-render", LAB / "bm-java/web", LAB / "bm-rs/web", "--map", map_id, "--inset", "16",
             "--json", LAB / f"diff-{map_id}.json", "--top", args.top])
        run([sys.executable, Path(__file__).parent / "lowres_diff.py", LAB / "bm-java/web/maps" / map_id,
             LAB / "bm-rs/web/maps" / map_id])


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("step", nargs="?", default="all")
    ap.add_argument("--radius", default="8")
    ap.add_argument("--quiet", default="15")
    ap.add_argument("--limit", default="300")
    ap.add_argument("--top", default="30")
    ap.add_argument("--memory", help="crawl store memory limit in bytes (0: neighbours come back from disk)")
    ap.add_argument("--no-scene", dest="scene", action="store_false", help="crawl natural terrain only")
    ap.add_argument("--dims", default="overworld", help="dimensions to crawl, comma-separated, or all")
    args = ap.parse_args()
    args.dims = list(DIMS) if args.dims == "all" else args.dims.split(",")
    if args.step in ("all", "server", "crawl"):
        server = start_server()
        try:
            if args.scene:
                for cmd in scene.commands(REPORT):
                    server.command(cmd)
                server.command("save-all flush")
                server.wait_for("Saved the game", timeout=600)
            # chunks generated while the bot looks (nether lava falls) must not flow on after it has left
            server.command("tick freeze")
            if args.step != "server":
                crawl(args)
            else:
                input("server up; enter stops it\n")
        finally:
            server.stop()
    if args.step in ("all", "trim"):
        trim()
    if args.step in ("all", "golden"):
        golden()
    if args.step in ("all", "candidate"):
        candidate()
    if args.step in ("all", "diff"):
        diff(args)


if __name__ == "__main__":
    sys.exit(main())
