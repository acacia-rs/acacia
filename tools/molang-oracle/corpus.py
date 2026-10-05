"""Collects the Molang expressions of the vanilla packs into assets/molang/corpus.txt. See README.md.

usage: corpus.py [--tag TAG] [--samples DIR] [--out FILE]
"""
import argparse
import json
import re
import subprocess
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
PREFIX = r"(?:query|q|variable|v|temp|t|context|c|math|array|geometry|texture|material)"
MOLANG = re.compile(rf"(?<![\w./:]){PREFIX}\.[a-z_]", re.I)
RESOURCE_NAME = re.compile(r"^(geometry|texture|material)\.[\w.:]+$", re.I)
COMMENT = re.compile(r'("(?:\\.|[^"\\])*")|//[^\n]*|/\*.*?\*/', re.S)
TRAILING_COMMA = re.compile(r",(\s*[}\]])")
# Entity script arrays the game runs as one program: a statement may span entries.
JOINED = {"pre_animation", "initialize"}


def load(path):
    text = COMMENT.sub(lambda found: found.group(1) or "", path.read_text(encoding="utf-8-sig", errors="replace"))
    try:
        return json.loads(text)
    except json.JSONDecodeError:
        return json.loads(TRAILING_COMMA.sub(r"\1", text))


def strings(node, key=None):
    if isinstance(node, str):
        yield node
    elif isinstance(node, list):
        if key in JOINED and all(isinstance(item, str) for item in node):
            yield " ".join(node)
        else:
            for item in node:
                yield from strings(item)
    elif isinstance(node, dict):
        for name, value in node.items():
            yield from strings(value, name)


def fetch(tag, into):
    git = ["git", "-C", str(into)]
    subprocess.run(["git", "clone", "-q", "--depth", "1", "--branch", tag, "--filter=blob:none", "--no-checkout",
                    "https://github.com/Mojang/bedrock-samples.git", str(into)], check=True)
    subprocess.run([*git, "sparse-checkout", "set", "--no-cone", "/resource_pack/**/*.json", "/behavior_pack/**/*.json"], check=True)
    subprocess.run([*git, "checkout", "-q", "HEAD"], check=True)


def collect(samples):
    found = {}
    unreadable = 0
    for pack in ("resource_pack", "behavior_pack"):
        for path in sorted((samples / pack).rglob("*.json")):
            try:
                document = load(path)
            except (json.JSONDecodeError, OSError):
                unreadable += 1
                continue
            for text in strings(document):
                # Commands and bare resource names share fields with Molang.
                if text.startswith(("/", "@")) or RESOURCE_NAME.match(text.strip()) or not MOLANG.search(text):
                    continue
                found.setdefault(" ".join(text.split()))
    return list(found), unreadable


def main():
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--tag", default="v1.26.50.4", help="bedrock-samples tag to fetch")
    parser.add_argument("--samples", type=Path, help="an existing bedrock-samples checkout, instead of fetching")
    parser.add_argument("--out", type=Path, default=HERE.parents[1] / "assets" / "molang" / "corpus.txt")
    args = parser.parse_args()

    with tempfile.TemporaryDirectory() as scratch:
        samples = args.samples
        if samples is None:
            samples = Path(scratch) / "samples"
            fetch(args.tag, samples)
        expressions, unreadable = collect(samples)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text("\n".join(expressions) + "\n", encoding="utf-8", newline="\n")
    print(f"{len(expressions)} distinct expressions -> {args.out} ({unreadable} unreadable files)")


if __name__ == "__main__":
    main()
