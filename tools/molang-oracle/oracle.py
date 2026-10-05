"""Asks a Bedrock Dedicated Server what Molang expressions evaluate to. See README.md.

usage: oracle.py [--bds DIR] [--timeout SECONDS] <file.cases>...
"""
import argparse
import json
import os
import re
import shutil
import struct
import subprocess
import sys
import threading
from pathlib import Path

HERE = Path(__file__).resolve().parent
PACK_ID = json.loads((HERE / "pack" / "manifest.json").read_text(encoding="utf-8"))["header"]["uuid"]
PER_TYPE = 200
UNSET = -777777
STEP = "q.property('oracle:step')"
SETTINGS = {
    "server-port": "19180",
    "server-portv6": "19181",
    "level-name": "oracle",
    "level-type": "FLAT",
    "online-mode": "false",
    "content-log-console-output-enabled": "true",
}


class Case:
    """One line of a cases file: `expr`, `statements => expr` or `truthy: complex expression`."""

    def __init__(self, line):
        self.line = line
        self.setup = self.condition = None
        if line.startswith("truthy: "):
            self.condition = line.removeprefix("truthy: ")
        elif " => " in line:
            self.setup, self.read = line.split(" => ", 1)
        else:
            self.read = line

    @property
    def stateful(self):
        return self.setup is not None or self.condition is not None


def read_cases(path):
    """The file's cases, and the engine versions a `# engines: 1.13.0 1.18.10` line asks for."""
    lines = [line.strip() for line in path.read_text(encoding="utf-8").splitlines()]
    engines = next((line.split()[2:] for line in lines if line.startswith("# engines:")), None)
    return [Case(line) for line in lines if line and not line.startswith("#")], engines


def default_bds():
    """`.testserver` lives in the main checkout only, which sits beside a worktree as `acacia`."""
    root = HERE.parents[1]
    found = [base / ".testserver" / "bds-molang" for base in (root, root.parent / "acacia")]
    return os.environ.get("MOLANG_BDS") or next((path for path in found if path.exists()), found[0])


def probe_files(cases, first):
    """The entity and animation controller for the cases numbered from `first`."""
    name = f"probe{first // PER_TYPE}"
    numbered = list(enumerate(cases, first))
    states = {"default": {"transitions": [{f"s{i}": f"{STEP} == {i}"} for i, case in numbered if case.stateful]}}
    events = {}
    for i, case in numbered:
        leave = {"default": f"{STEP} != {i}"}
        read = case.read if case.condition is None else f"v.hit{i} ?? 0"
        if case.setup is not None:
            states[f"s{i}"] = {"on_entry": [case.setup], "transitions": [leave]}
        elif case.condition is not None:
            states[f"s{i}"] = {"transitions": [{f"h{i}": case.condition}, leave]}
            states[f"h{i}"] = {"on_entry": [f"v.hit{i} = 1;"], "transitions": [leave]}
        events[f"oracle:e{i}"] = {"set_property": {"oracle:out": read}}
    controller = f"controller.animation.oracle.{name}"
    entity = {
        "format_version": "1.21.90",
        "minecraft:entity": {
            "description": {
                "identifier": f"oracle:{name}",
                "is_spawnable": False,
                "is_summonable": True,
                "properties": {
                    "oracle:out": {"type": "float", "range": [-1e30, 1e30], "default": float(UNSET)},
                    "oracle:step": {"type": "int", "range": [-1, first + PER_TYPE], "default": -1},
                },
                "animations": {"oracle": controller},
                "scripts": {"animate": ["oracle"]},
            },
            "components": {"minecraft:collision_box": {"width": 0.1, "height": 0.1}},
            "events": events,
        },
    }
    controllers = {"format_version": "1.10.0", "animation_controllers": {controller: {"initial_state": "default", "states": states}}}
    return name, entity, controllers


def install(bds, cases, engine):
    """Replaces the instance's oracle pack and world, and points server.properties at them."""
    pack = bds / "development_behavior_packs" / "molang_oracle"
    world = bds / "worlds" / "oracle"
    for stale in (pack, world):
        shutil.rmtree(stale, ignore_errors=True)
    shutil.copytree(HERE / "pack", pack)
    manifest = json.loads((pack / "manifest.json").read_text(encoding="utf-8"))
    manifest["header"]["min_engine_version"] = [int(part) for part in engine.split(".")]
    (pack / "manifest.json").write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    (pack / "entities").mkdir()
    (pack / "animation_controllers").mkdir()
    for first in range(0, len(cases), PER_TYPE):
        name, entity, controllers = probe_files(cases[first : first + PER_TYPE], first)
        (pack / "entities" / f"{name}.json").write_text(json.dumps(entity, indent=1), encoding="utf-8")
        (pack / "animation_controllers" / f"{name}.json").write_text(json.dumps(controllers, indent=1), encoding="utf-8")
    stateful = [i for i, case in enumerate(cases) if case.stateful]
    constants = f"export const COUNT = {len(cases)};\nexport const PER_TYPE = {PER_TYPE};\nexport const STATEFUL = {stateful};\n"
    (pack / "scripts" / "cases.js").write_text(constants, encoding="utf-8")
    world.mkdir(parents=True)
    (world / "world_behavior_packs.json").write_text(json.dumps([{"pack_id": PACK_ID, "version": [1, 0, 0]}]), encoding="utf-8")

    props = bds / "server.properties"
    lines = [line for line in props.read_text(encoding="utf-8").splitlines() if line.split("=", 1)[0] not in SETTINGS]
    props.write_text("\n".join(lines + [f"{key}={value}" for key, value in SETTINGS.items()]) + "\n", encoding="utf-8")


def run_server(bds, timeout):
    """Runs BDS until the pack logs `ORACLE done`; returns its console lines."""
    windows = os.name == "nt"
    exe = bds / ("bedrock_server.exe" if windows else "bedrock_server")
    env = os.environ if windows else {**os.environ, "LD_LIBRARY_PATH": "."}
    server = subprocess.Popen(
        [str(exe)], cwd=bds, env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, encoding="utf-8", errors="replace"
    )
    watchdog = threading.Timer(timeout, server.kill)
    watchdog.start()
    lines = []
    for line in server.stdout:
        lines.append(line.rstrip())
        if "ORACLE done" in line:
            server.stdin.write("stop\n")
            server.stdin.flush()
    server.wait()
    watchdog.cancel()
    (bds / "oracle.log").write_text("\n".join(lines) + "\n", encoding="utf-8")
    return lines


def shortest_f32(text):
    """The shortest decimal that reads back as the same f32 (the script prints f32s widened to f64)."""
    value = float(text)
    as_f32 = struct.pack("<f", value)
    for digits in range(1, 10):
        short = f"{value:.{digits}g}"
        if struct.pack("<f", float(short)) == as_f32:
            return short if "e" not in short or abs(value) >= 1e9 or abs(value) < 1e-4 else f"{float(short):.9g}"
    return text


def collect(lines, count):
    """Per case: (result, Molang error messages BDS logged for it)."""
    results = ["missing"] * count
    errors = [[] for _ in range(count)]
    unplaced = []
    for line in lines:
        if found := re.search(r"ORACLE (\d+) (.*)", line):
            index, value = int(found[1]), found[2]
            results[index] = "error" if value.startswith("error") else "unset" if float(value) == UNSET else shortest_f32(value)
        elif "[Molang]" in line:
            where = re.search(r"\| (?:oracle:e|s|h)(\d+) \|", line)
            message = line.rsplit(" | ", 1)[-1]
            if where and int(where[1]) < count:
                errors[int(where[1])].append(message)
            else:
                unplaced.append(line)
    return results, errors, unplaced


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("files", nargs="+", type=Path)
    parser.add_argument("--bds", type=Path, default=Path(default_bds()))
    parser.add_argument("--timeout", type=float, default=300)
    args = parser.parse_args()
    default_engine = ".".join(map(str, json.loads((HERE / "pack" / "manifest.json").read_text(encoding="utf-8"))["header"]["min_engine_version"]))

    files = [(path, *read_cases(path)) for path in args.files]
    engines = dict.fromkeys(engine for _, _, asked in files for engine in asked or [default_engine])
    missing = 0
    for engine in engines:
        chosen = [(path, cases, asked) for path, cases, asked in files if engine in (asked or [default_engine])]
        cases = [case for _, file_cases, _ in chosen for case in file_cases]
        install(args.bds, cases, engine)
        lines = run_server(args.bds, args.timeout)
        version = next((line.split("Version: ")[1] for line in lines if "Version: " in line), "unknown")
        results, errors, unplaced = collect(lines, len(cases))

        first = 0
        for path, file_cases, asked in chosen:
            rows = [f"# BDS {version}, engine {engine}; result, case, Molang errors. Written by tools/molang-oracle/oracle.py."]
            for i, case in enumerate(file_cases, first):
                rows.append("\t".join([results[i], case.line, *dict.fromkeys(errors[i])]))
            first += len(file_cases)
            out = path.with_name(f"{path.stem}@{engine}.bds") if asked else path.with_suffix(".bds")
            out.write_text("\n".join(rows) + "\n", encoding="utf-8", newline="\n")

        counts = {kind: sum(1 for r in results if r == kind) for kind in ("error", "unset", "missing")}
        missing += counts["missing"]
        print(f"BDS {version}, engine {engine}: {len(cases)} cases, {counts['error']} rejected, {counts['unset']} unset, {counts['missing']} missing")
        for line in unplaced[:20]:
            print("unplaced:", line[:300])
    return 1 if missing else 0


if __name__ == "__main__":
    sys.exit(main())
