#!/usr/bin/env python3
"""Runs parity harnesses against the Java oracle home (docs/parity.md).

  python3 tools/parity/parity.py              every harness
  python3 tools/parity/parity.py bake light   these
  python3 tools/parity/parity.py --list
  options: --fresh (rerun cached oracle runs), --detail N (divergences shown per harness, default 20)

Prints each harness's cases and divergences (case | field | expected | got) and exits 1 if any
harness diverges or fails. Results: ~/java-truth/runs/<name>/ (log.txt, divergences.tsv, result.json).
From Windows: tools/parity/remote.sh <same arguments> (runs here on devbox)."""
import argparse
import importlib.util
import json
from pathlib import Path
import sys
import time
import traceback

sys.path.insert(0, str(Path(__file__).resolve().parent))
from paritylib.harness import Harness  # noqa: E402
from paritylib.home import PARITY  # noqa: E402

HARNESSES = PARITY / "harnesses"


def load(name):
    spec = importlib.util.spec_from_file_location(f"harness_{name}", HARNESSES / name / "harness.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def available():
    return sorted(p.parent.name for p in HARNESSES.glob("*/harness.py"))


def run_one(name, fresh, detail):
    module = load(name)
    h = Harness(name, HARNESSES / name, fresh)
    started, error = time.monotonic(), None
    try:
        module.run(h)
    except Exception as e:  # a failed harness is reported, the others still run
        error = f"{type(e).__name__}: {e}"
        traceback.print_exc(file=h.log)
    seconds = time.monotonic() - started
    h.log.close()
    (h.out / "divergences.tsv").write_text("".join("\t".join(d) + "\n" for d in h.divergences), encoding="utf-8")
    result = dict(name=name, owner=getattr(module, "OWNER", "?"), cases=h.cases, divergences=len(h.divergences),
                  error=error, seconds=round(seconds, 1))
    (h.out / "result.json").write_text(json.dumps(result, indent=1))
    status = "FAIL" if error else ("DIVERGES" if h.divergences else "ok")
    print(f"{name}: {status}, {h.cases:,} cases, {len(h.divergences):,} divergences ({seconds:.0f} s)")
    if error:
        print(f"  {error}\n  log: {h.out / 'log.txt'}")
    for case, field, expected, got in h.divergences[:detail]:
        print(f"  {case} | {field} | expected {expected} | got {got}")
    if len(h.divergences) > detail:
        print(f"  ... {len(h.divergences) - detail} more in {h.out / 'divergences.tsv'}")
    return not error and not h.divergences


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("names", nargs="*")
    parser.add_argument("--list", action="store_true")
    parser.add_argument("--fresh", action="store_true")
    parser.add_argument("--detail", type=int, default=20)
    args = parser.parse_args()
    names = available()
    if args.list:
        for name in names:
            module = load(name)
            print(f"{name}\t{getattr(module, 'OWNER', '?')}\t{(module.__doc__ or '').strip().splitlines()[0]}")
        return 0
    unknown = set(args.names) - set(names)
    if unknown:
        sys.exit(f"unknown harness {', '.join(sorted(unknown))}; have {', '.join(names)}")
    ok = [run_one(name, args.fresh, args.detail) for name in (args.names or names)]
    return 0 if all(ok) else 1


if __name__ == "__main__":
    sys.exit(main())
