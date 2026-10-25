"""Compiling and running Java oracles, with both steps cached in the oracle home.

Classes are keyed by the content of every source compiled (the shared bootstrap and the
harness's own) and the pinned client jar; an oracle run by its classes, main class, arguments and
the content of its input files. A cached run is reused unless `fresh`."""
import hashlib
import os
from pathlib import Path
import shutil
import subprocess

from .home import HOME, JAVA, PARITY, PIN, classpath

SHARED = PARITY / "java"


def _hash(*parts):
    h = hashlib.sha256()
    for part in parts:
        h.update(part if isinstance(part, bytes) else str(part).encode())
        h.update(b"\0")
    return h.hexdigest()[:20]


def sources(harness_dir):
    return sorted(SHARED.rglob("*.java")) + sorted(Path(harness_dir).rglob("*.java"))


def compile_classes(harness_dir, log):
    files = sources(harness_dir)
    key = _hash(PIN["minecraft"]["client_sha1"], *(p.relative_to(PARITY).as_posix().encode() + p.read_bytes() for p in files))
    out = HOME / "cache" / "classes" / key
    if (out / ".done").is_file():
        return out, key
    shutil.rmtree(out, ignore_errors=True)
    out.mkdir(parents=True)
    run([JAVA / "javac", "--release", str(PIN["minecraft"]["java_major"]), "-proc:none", "-nowarn", "-encoding", "UTF-8",
         "-cp", classpath(), "-d", out, *files], log)
    (out / ".done").touch()
    return out, key


def run_oracle(harness_dir, name, main, args, inputs, fresh, log, heap="4g"):
    """Runs `main <out dir> <args...>`; returns the out dir holding what it wrote."""
    classes, key = compile_classes(harness_dir, log)
    run_key = _hash(key, main, *args, *(Path(i).read_bytes() for i in inputs))
    out = HOME / "cache" / "oracle" / name / run_key
    if (out / ".done").is_file() and not fresh:
        log.write(f"oracle {main}: reusing {out}\n")
        return out
    shutil.rmtree(out, ignore_errors=True)
    out.mkdir(parents=True)
    run([JAVA / "java", f"-Xmx{heap}", "-Djava.awt.headless=true", "--enable-native-access=ALL-UNNAMED",
         "-cp", f"{classes}{os.pathsep}{classpath()}", main, out, *args], log, cwd=out)
    (out / ".done").touch()
    return out


def run(command, log, cwd=None):
    command = [str(c) for c in command]
    log.write("$ " + " ".join(command[:6]) + (" ..." if len(command) > 6 else "") + "\n")
    log.flush()
    result = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, cwd=cwd)
    if result.returncode:
        raise RuntimeError(f"{Path(command[0]).name} exited {result.returncode}; see {log.name}")
