"""Where the oracle home and the repo are, and the pins every tool reads (tools/parity/lock.json)."""
import json
import os
from pathlib import Path

PARITY = Path(__file__).resolve().parents[1]
ROOT = PARITY.parents[1]
PIN = json.loads((PARITY / "lock.json").read_text())
HOME = Path(os.environ.get("JAVA_TRUTH", Path.home() / "java-truth"))
VERSION = PIN["minecraft"]["version"]
GAME = HOME / "versions" / VERSION
JAVA = HOME / "jdk" / "bin"


def classpath():
    path = GAME / "classpath.txt"
    if not path.is_file():
        raise SystemExit(f"no oracle home at {HOME}: run python3 tools/parity/setup.py")
    return path.read_text().strip()
