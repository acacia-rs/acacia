"""The context a harness's `run(h)` gets (docs/parity.md "Adding a harness")."""
import os
from pathlib import Path
import subprocess

from . import compare, oracle
from .home import HOME, JAVA, ROOT


class Harness:
    def __init__(self, name, directory, fresh):
        self.name, self.dir, self.fresh = name, Path(directory), fresh
        self.out = HOME / "runs" / name
        self.out.mkdir(parents=True, exist_ok=True)
        self.log = open(self.out / "log.txt", "w", encoding="utf-8")
        self.cases = 0
        self.divergences = []

    def oracle(self, main, *args, inputs=(), heap="4g"):
        """Compiles tools/parity/java and this harness's *.java, runs `main <out dir> <args>`, returns the out dir."""
        return oracle.run_oracle(self.dir, self.name, main, [str(a) for a in args], inputs, self.fresh, self.log, heap)

    def cargo(self, *args, check=True):
        """`cargo <args>` in the repo; returns its stdout (stderr goes to the log)."""
        command = ["cargo", *map(str, args)]
        self.log.write("$ " + " ".join(command) + "\n")
        self.log.flush()
        # lookbake runs its Java exporters with the oracle's JDK.
        env = dict(os.environ, CARGO_TERM_COLOR="never")
        env.setdefault("JAVA_HOME", str(JAVA.parent))
        result = subprocess.run(command, cwd=ROOT, stdout=subprocess.PIPE, stderr=self.log, text=True, env=env)
        self.log.write(result.stdout)
        if check and result.returncode:
            raise RuntimeError(f"cargo exited {result.returncode}; see {self.log.name}")
        return result.stdout

    def probe(self, package, example, *args):
        """Runs a CPU-side Rust probe: `cargo run --release -p <package> --example <example> -- <args>`."""
        return self.cargo("run", "--release", "-q", "-p", package, "--example", example, "--", *args)

    def diverge(self, case, field, expected, got):
        self.divergences.append(tuple(str(x) for x in (case, field, expected, got)))

    def add(self, cases, divergences):
        self.cases += cases
        for d in divergences:
            self.diverge(*d)

    def compare_jsonl(self, expected, got, key="case", tolerance=1e-4, first_per_case=3):
        """The oracle's and the probe's JSON-lines records, field by field (paritylib.compare.jsonl)."""
        self.add(*compare.jsonl(expected, got, key, tolerance, first_per_case))

    def read_probe(self, text):
        """Takes a self-comparing probe's CASES / DIVERGENCE lines."""
        cases, divergences = compare.parse_probe(text)
        if cases is None:
            raise RuntimeError("the probe printed no CASES line")
        self.add(cases, divergences)
