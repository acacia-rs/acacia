"""bluemap-rs `-u` against a world acacia-anvil is writing (README.md "Live updates"): a synthetic store
(`examples/live.rs`) and bluemap-rs watching it, checking that

  a. a changed block re-renders exactly the tiles over its chunk,
  b. an unchanged rewrite renders nothing (no file write at all; or, rewritten beside a changed chunk,
     only the changed chunk's tiles),
  c. regions replaced by temp file + rename under load are picked up and never read half-written: a reader
     thread parses every version it sees, bluemap logs no warning, and its incremental map equals a fresh
     full render of the final world (bm-golden diff-render, lowres pixels).

Usage: py -3 crates/acacia-anvil/tools/live.py [--side 40] [--compression zlib|lz4|none]. Uses the oracle lab
(oracle.py) for paths.
"""
import argparse
import json
import os
import random
import re
import shutil
import subprocess
import sys
import threading
import time
from pathlib import Path

import oracle
from regions import payload_ok

ROOT = oracle.LAB / "live"
MAP_LINE = re.compile(r"Map 'lab': \d+ regions, (\d+) tiles rendered")
TILE = re.compile(r"x(-?\d+)z(-?\d+)")


class Logged:
    """A process whose output lines are kept (and logged) as they come."""

    def __init__(self, cmd, cwd, log, stdin=None):
        self.lines, self.times = [], []
        self.proc = subprocess.Popen([str(c) for c in cmd], cwd=cwd, stdin=stdin, stdout=subprocess.PIPE,
                                     stderr=subprocess.STDOUT, text=True, encoding="utf-8", errors="replace")
        self.log = open(log, "w", encoding="utf-8")
        threading.Thread(target=self._read, daemon=True).start()

    def _read(self):
        for line in self.proc.stdout:
            self.times.append(time.monotonic())
            self.lines.append(line.rstrip("\n"))
            self.log.write(line)
            self.log.flush()

    def wait_for(self, text, timeout=300, since=0):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            if any(text in l for l in self.lines[since:]):
                return
            if self.proc.poll() is not None:
                raise SystemExit(f"process exited waiting for {text!r}")
            time.sleep(0.2)
        raise SystemExit(f"timed out waiting for {text!r}")

    def settle(self, since, quiet=8.0, timeout=180):
        """Waits for a region update after line `since`, then for `quiet` s without another; tiles rendered."""
        self.wait_for("tiles rendered", timeout, since)
        seen, last = len(self.lines), time.monotonic()
        while time.monotonic() - last < quiet:
            time.sleep(0.2)
            if len(self.lines) != seen:
                seen, last = len(self.lines), time.monotonic()
        return sum(int(m.group(1)) for l in self.lines[since:] if (m := MAP_LINE.search(l)))

    def renders(self, since):
        """(scheduled, finished) times of each region update after line `since`."""
        out, start = [], None
        for t, l in zip(self.times[since:], self.lines[since:]):
            if "Scheduled update" in l and start is None:
                start = t
            elif MAP_LINE.search(l) and start is not None:
                out.append((start, t))
                start = None
        return out


class Writer:
    def __init__(self, exe, world):
        self.proc = subprocess.Popen([str(exe), str(world)], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                     text=True)

    def cmd(self, line):
        self.proc.stdin.write(line + "\n")
        self.proc.stdin.flush()
        reply = self.proc.stdout.readline().strip()
        if not reply.startswith("ok"):
            raise SystemExit(f"writer: {line!r} -> {reply!r}")
        return reply


def snapshot(map_dir):
    tiles = map_dir / "tiles/0"
    return {p.relative_to(tiles).as_posix(): p.read_bytes() for p in tiles.rglob("*.prbm*")}


def tile(path):
    m = TILE.search(path.replace("/", ""))
    return int(m.group(1)), int(m.group(2))


def changed(before, after):
    return {tile(p) for p in before.keys() | after.keys() if before.get(p) != after.get(p)}


def tiles_of_chunk(cx, cz):
    """Hires tiles (32 blocks, offset 2) overlapping a chunk: the ones BlueMap re-renders when it changes."""
    span = lambda c: range((c * 16 - 2) // 32, (c * 16 + 15 - 2) // 32 + 1)
    return {(x, z) for x in span(cx) for z in span(cz)}


def region_ok(data, kind):
    """Every chunk the offset table lists lies inside the file, has compression `kind` and is whole."""
    if len(data) < 8192 or len(data) % 4096:
        return False
    for i in range(1024):
        loc = int.from_bytes(data[i * 4:i * 4 + 4], "big")
        if not loc:
            continue
        off = (loc >> 8) * 4096
        length = int.from_bytes(data[off:off + 4], "big")
        if off + 4 + length > len(data) or data[off + 4] != kind or not payload_ok(data[off + 4:off + 4 + length]):
            return False
    return True


class Reader(threading.Thread):
    def __init__(self, paths, kind):
        super().__init__(daemon=True)
        self.paths, self.kind, self.reads, self.bad, self.stop = paths, kind, 0, [], False

    def run(self):
        while not self.stop:
            for p in self.paths:
                try:
                    data = p.read_bytes()
                except OSError:
                    continue
                self.reads += 1
                if not region_ok(data, self.kind):
                    self.bad.append(p.name)
            time.sleep(0.003)


def bluemap_config(name, world):
    base = oracle.bluemap_dir(name, world)
    core = base / "config/core.conf"
    core.write_text(re.sub(r"(?m)^update-cooldown: \d+", "update-cooldown: 1", core.read_text()))
    return base


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--side", type=int, default=40, help="chunks per side (40 spans four regions)")
    ap.add_argument("--compression", default="zlib", help="the store's chunk compression: zlib[:level], lz4, none")
    args = ap.parse_args()
    os.environ["ANVIL_COMPRESSION"] = args.compression
    kind = {"zlib": 2, "none": 3, "lz4": 4}[args.compression.split(":")[0]]
    oracle.run(["cargo", "build", "--release", "-p", "acacia-anvil", "--example", "live"], cwd=oracle.REPO)
    bin_dir = oracle.build_bluemap_rs()
    exe = lambda n: bin_dir / (n + (".exe" if sys.platform == "win32" else ""))
    shutil.rmtree(ROOT, ignore_errors=True)
    ROOT.mkdir(parents=True)
    world = ROOT / "world"
    region_dir = world / oracle.OVERWORLD
    writer = Writer(oracle.REPO / "target/release/examples/live", world)
    print(writer.cmd(f"init {args.side}"))

    base = bluemap_config("bm-live", world)
    map_dir = base / "web/maps/lab"
    bm = Logged([exe("bluemap"), "-c", "config", "-v", "26.3", "-u"], base, ROOT / "bluemap.log")
    results = []

    def check(name, ok, detail):
        results.append(ok)
        print(f"{'PASS' if ok else 'FAIL'} {name}: {detail}", flush=True)

    try:
        bm.wait_for("Your maps are now all up-to-date!")
        bm.wait_for("Started watching map")

        before, mark = snapshot(map_dir), len(bm.lines)
        writer.cmd("set 104 75 104 minecraft:gold_block")
        writer.cmd("flush")
        rendered = bm.settle(mark)
        diff, want = changed(before, snapshot(map_dir)), tiles_of_chunk(6, 6)
        check("a. changed block", rendered == len(want) and diff <= want and (3, 3) in diff,
              f"{rendered} tiles rendered (want {len(want)}), files changed {sorted(diff)}")

        r00 = region_dir / "r.0.0.mca"
        file_before, before, mark = r00.read_bytes(), snapshot(map_dir), len(bm.lines)
        writer.cmd("touch 6 6")
        writer.cmd("touch 20 20")
        reply = writer.cmd("flush")
        time.sleep(10)
        scheduled = [l for l in bm.lines[mark:] if "Scheduled update" in l]
        check("b. unchanged rewrite", r00.read_bytes() == file_before and not scheduled
              and not changed(before, snapshot(map_dir)), f"{reply}; region file untouched, {len(scheduled)} updates")

        before, mark = snapshot(map_dir), len(bm.lines)
        writer.cmd("set 136 75 104 minecraft:gold_block")
        writer.cmd("touch 20 20")
        writer.cmd("flush")
        rendered = bm.settle(mark)
        diff, want = changed(before, snapshot(map_dir)), tiles_of_chunk(8, 6)
        check("b. rewrite beside a change", r00.read_bytes() != file_before and rendered == len(want)
              and diff <= want, f"{rendered} tiles rendered (want {len(want)}), files changed {sorted(diff)}")

        reader = Reader(sorted(region_dir.glob("r.*.mca")), kind)
        reader.start()
        rng, mark = random.Random(1), len(bm.lines)
        blocks = ["minecraft:gold_block", "minecraft:glowstone", "minecraft:stone", "minecraft:air"]
        top, writes = args.side * 16 - 16, []
        for burst in range(4):
            # each burst after the first starts as a region update does, so writes land while bluemap reads
            if burst:
                bm.wait_for("Scheduled update", since=quiet_from)
            for _ in range(12):
                x, z = rng.randrange(16, top), rng.randrange(16, top)
                writer.cmd(f"set {x} {rng.randrange(70, 80)} {z} {rng.choice(blocks)}")
                writer.cmd("flush")
                writes.append(time.monotonic())
                time.sleep(0.1)
            quiet_from = len(bm.lines)
        bm.settle(mark, quiet=12)
        reader.stop = True
        reader.join()
        renders = bm.renders(mark)
        during = sum(1 for w in writes if any(a <= w <= b for a, b in renders))
        leftovers = list(region_dir.glob("*.tmp"))
        check("c. reads during rewrites", reader.reads > 0 and not reader.bad and not leftovers,
              f"{reader.reads} region reads, {len(reader.bad)} torn, {len(leftovers)} temp files left, "
              f"{len(renders)} region updates, {during} region writes while one ran")
    finally:
        bm.proc.terminate()
        bm.proc.wait()
        writer.proc.stdin.close()
        writer.proc.wait()
    warnings = [l for l in bm.lines if re.search(r"\b(WARN|WARNING|ERROR)\b", l)]
    check("c. bluemap log", not warnings, f"{len(warnings)} warnings/errors" + "".join(f"\n  {l}" for l in warnings[:5]))

    fresh = bluemap_config("bm-live-fresh", world)
    oracle.run([exe("bluemap"), "-c", "config", "-v", "26.3", "-r", "-f"], cwd=fresh)
    out = ROOT / "diff.json"
    oracle.run([exe("bm-golden"), "diff-render", fresh / "web", base / "web", "--json", out])
    d = json.loads(out.read_text())
    same = d["identical"] == d["original_faces"] == d["candidate_faces"] and not d["missing"] and not d["extra"]
    check("c. incremental = fresh render", same, f"{d['identical']} of {d['original_faces']} faces identical")
    low = subprocess.run([sys.executable, Path(__file__).parent / "lowres_diff.py", fresh / "web/maps/lab",
                          map_dir], capture_output=True, text=True).stdout.strip().splitlines()
    check("c. lowres = fresh render", low and all(" 0/" in l for l in low), "; ".join(low))
    return 0 if all(results) else 1


if __name__ == "__main__":
    sys.exit(main())
