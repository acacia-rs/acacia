"""The acacia-anvil acceptance test (README.md "Oracle"): Java BlueMap's render of a Paper world against
bluemap-rs's render of what an acacia bot, joined through Geyser, saw of it.

Usage: py -3 crates/acacia-anvil/tools/oracle.py [all|server|crawl|trim|golden|candidate|diff] [--radius 8]

Lab: <repo>/.testserver/anvil-lab (git-ignored): Paper 26.3 + Geyser-Spigot (offline, Bedrock :19150),
fixed seed. Java, BlueMap 5.29 and the MC client jar are read from the bluemap-rs checkout (BLUEMAP_RS).
"""
import argparse
import json
import os
import shutil
import struct
import subprocess
import sys
import threading
import time
from pathlib import Path

import scene

REPO = Path(__file__).resolve().parents[3]
REPORT = Path(__file__).resolve().parent / ".cache/java-26.3-blocks.json"
LAB = REPO / ".testserver" / "anvil-lab"
SERVER = LAB / "server"
BMRS = Path(os.environ.get("BLUEMAP_RS", "C:/Users/hexay/bluemap-rs"))
JAVA = BMRS / "work/downloads/jdk25/bin" / ("java.exe" if os.name == "nt" else "java")
BLUEMAP_JAR = BMRS / "work/downloads/bluemap-5.29-cli.jar"
CLIENT_JAR = BMRS / "work/bluemap/vanilla/data/minecraft-client-26.3.jar"
MAP_TEMPLATE = BMRS / "work/bluemap/vanilla/config/maps/vanilla.conf"
# bluemap-rs binaries built from that checkout into the lab (its own target dir stays untouched)
BM_TARGET = LAB / "bm-target"
OVERWORLD = "dimensions/minecraft/overworld/region"
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
    run(["cargo", "run", "--release", "-p", "acacia-anvil", "--example", "crawl", "--",
         f"127.0.0.1:{GEYSER_PORT}", cand, "Anvil", args.radius, args.quiet, args.limit], cwd=REPO)


def server_region_dir():
    for d in [SERVER / "world" / OVERWORLD, SERVER / "world/region"]:
        if d.is_dir():
            return d
    raise SystemExit("no server region dir")


def read_region(path):
    data = path.read_bytes()
    chunks = {}
    for i in range(1024):
        loc = struct.unpack_from(">I", data, i * 4)[0]
        off, count = (loc >> 8) * 4096, loc & 0xFF
        if count and off + 4 <= len(data):
            length = struct.unpack_from(">I", data, off)[0]
            ts = struct.unpack_from(">I", data, 4096 + i * 4)[0]
            chunks[i] = (data[off + 4: off + 4 + length], ts)
    return chunks


def write_region(path, chunks):
    header, body, sector = bytearray(8192), bytearray(), 2
    for i, (payload, ts) in sorted(chunks.items()):
        blob = struct.pack(">I", len(payload)) + payload
        blob += b"\0" * (-len(blob) % 4096)
        struct.pack_into(">I", header, i * 4, (sector << 8) | (len(blob) // 4096))
        struct.pack_into(">I", header, 4096 + i * 4, ts)
        body += blob
        sector += len(blob) // 4096
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(bytes(header + body))


def trim():
    """The server's world cut to the chunks the bot saw, so both renders cover one area."""
    cand = LAB / "candidate/world" / OVERWORLD
    out = LAB / "golden/world" / OVERWORLD
    shutil.rmtree(LAB / "golden/world", ignore_errors=True)
    kept = 0
    for f in sorted(cand.glob("r.*.mca")):
        keep = read_region(f).keys()
        src = server_region_dir() / f.name
        chunks = {i: c for i, c in read_region(src).items() if i in keep} if src.exists() else {}
        missing = len(keep) - len(chunks)
        if missing:
            print(f"{f.name}: {missing} chunks the server never saved")
        write_region(out / f.name, chunks)
        kept += len(chunks)
    print(f"golden world: {kept} chunks")


def bluemap_dir(name, world):
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
    conf = MAP_TEMPLATE.read_text().splitlines()
    conf = [f'world: {json.dumps(world.as_posix())}' if l.startswith("world:") else
            'name: "lab"' if l.startswith("name:") else l for l in conf]
    (cfg / "maps/lab.conf").write_text("\n".join(conf) + "\n")
    shutil.rmtree(base / "web/maps", ignore_errors=True)
    return base


def build_bluemap_rs():
    run(["cargo", "build", "--profile", "fast", "--locked", "-j", "4", "-p", "bm-cli", "-p", "bm-golden"],
        cwd=BMRS, env={**os.environ, "CARGO_TARGET_DIR": str(BM_TARGET)})
    return BM_TARGET / "fast"


def golden():
    base = bluemap_dir("bm-java", LAB / "golden/world")
    run([JAVA, "-jar", BLUEMAP_JAR, "-c", "config", "-v", "26.3", "-r", "-f"], cwd=base)


def candidate():
    exe = build_bluemap_rs() / ("bluemap.exe" if os.name == "nt" else "bluemap")
    base = bluemap_dir("bm-rs", LAB / "candidate/world")
    run([exe, "-c", "config", "-v", "26.3", "-r", "-f"], cwd=base)


def diff(args):
    exe = build_bluemap_rs() / ("bm-golden.exe" if os.name == "nt" else "bm-golden")
    run([exe, "diff-render", LAB / "bm-java/web", LAB / "bm-rs/web", "--inset", "16", "--json",
         LAB / "diff.json", "--top", args.top])


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("step", nargs="?", default="all")
    ap.add_argument("--radius", default="8")
    ap.add_argument("--quiet", default="15")
    ap.add_argument("--limit", default="300")
    ap.add_argument("--top", default="30")
    ap.add_argument("--no-scene", dest="scene", action="store_false", help="crawl natural terrain only")
    args = ap.parse_args()
    if args.step in ("all", "server", "crawl"):
        server = start_server()
        try:
            if args.scene:
                for cmd in scene.commands(REPORT):
                    server.command(cmd)
                server.command("save-all flush")
                server.wait_for("Saved the game", timeout=600)
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
