#!/usr/bin/env python3
"""Builds the Java oracle home (default ~/java-truth, or $JAVA_TRUTH) on a Linux x86-64 host.

Fetches a pinned JDK, the pinned Minecraft client and server jars and the client's libraries from
Mojang's version manifest (every file SHA-checked), unpacks the server bundle and decompiles the
client jar with Vineflower into sources/client for reading. 26.3 ships unobfuscated (no
client_mappings in its version JSON), so no remapping stage is needed. Idempotent: what is already
there and verified is kept. Nothing here is committed; see docs/parity.md.

  python3 tools/parity/setup.py [--no-decompile] [--threads N]
"""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import sys
import tarfile
import tempfile
import urllib.request
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parent))
from paritylib.home import HOME, PIN  # noqa: E402

MANIFEST = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"


def digest(path, algorithm):
    h = hashlib.new(algorithm)
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def fetch(url, path, checksum, algorithm="sha1"):
    """`path` when it has `checksum`, else downloaded there and checked."""
    path = Path(path)
    if path.is_file() and digest(path, algorithm) == checksum:
        return path
    path.parent.mkdir(parents=True, exist_ok=True)
    print(f"fetch {url}", flush=True)
    part = path.with_name(path.name + ".part")
    with urllib.request.urlopen(url, timeout=120) as response, open(part, "wb") as out:
        shutil.copyfileobj(response, out)
    got = digest(part, algorithm)
    if got != checksum:
        part.unlink()
        raise SystemExit(f"{url}: {algorithm} {got}, pinned {checksum}")
    part.rename(path)
    return path


def safe(relative):
    p = PurePosixPath(relative)
    if p.is_absolute() or ".." in p.parts or "\\" in relative:
        raise SystemExit(f"unsafe path {relative}")
    return Path(*p.parts)


def jdk():
    pin = PIN["jdk"]
    target = HOME / "jdk"
    if (target / "bin/java").is_file() and (target / "RELEASE_NAME").read_text().strip() == pin["release"]:
        return target / "bin/java"
    archive = fetch(pin["url"], HOME / "downloads" / Path(pin["url"]).name.replace("%2B", "+"), pin["sha256"], "sha256")
    stage = Path(tempfile.mkdtemp(dir=HOME, prefix="jdk-"))
    with tarfile.open(archive) as tar:
        tar.extractall(stage, filter="data")
    (top,) = stage.iterdir()
    shutil.rmtree(target, ignore_errors=True)
    top.rename(target)
    stage.rmdir()
    (target / "RELEASE_NAME").write_text(pin["release"] + "\n")
    return target / "bin/java"


def allowed(library):
    """The launcher's rules on Linux x86-64: the last matching rule decides; no rules means allowed."""
    rules = library.get("rules", [])
    ok = not rules
    for rule in rules:
        os_rule = rule.get("os", {})
        if os_rule.get("name", "linux") != "linux" or rule.get("features"):
            continue
        if "arch" in os_rule and os_rule["arch"] not in ("x86_64", "amd64", "x64"):
            continue
        ok = rule["action"] == "allow"
    name = library["name"].split(":")
    classifier = name[3] if len(name) > 3 else ""
    return ok and not classifier.endswith(("arm64", "aarch64", "aarch_64", "x86"))


def bundle(server, directory):
    """The server download is a launcher bundle: the inner game jar and its libraries, SHA-256 checked."""
    out = []
    with zipfile.ZipFile(server) as zf:
        for listing, prefix, dest in (("versions.list", "versions", directory),
                                      ("libraries.list", "libraries", directory / "libraries/server")):
            for line in zf.read(f"META-INF/{listing}").decode().splitlines():
                if not line.strip():
                    continue
                checksum, _, relative = line.split("\t")
                path = dest / ("server-game.jar" if prefix == "versions" else safe(relative))
                if not (path.is_file() and digest(path, "sha256") == checksum):
                    data = zf.read(f"META-INF/{prefix}/{relative}")
                    if hashlib.sha256(data).hexdigest() != checksum:
                        raise SystemExit(f"bundled {relative}: checksum mismatch")
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_bytes(data)
                out.append(path)
    return out


def game():
    pin = PIN["minecraft"]
    version = pin["version"]
    directory = HOME / "versions" / version
    with urllib.request.urlopen(MANIFEST, timeout=60) as response:
        entry = next(v for v in json.load(response)["versions"] if v["id"] == version)
    package = fetch(entry["url"], directory / "version.json", entry["sha1"])
    manifest = json.loads(package.read_text())
    downloads = manifest["downloads"]
    if downloads["client"]["sha1"] != pin["client_sha1"] or downloads["server"]["sha1"] != pin["server_sha1"]:
        raise SystemExit(f"Mojang's {version} names other jars than the pin; check before trusting it")
    if "client_mappings" in downloads:
        raise SystemExit(f"{version} is obfuscated; this setup has no remapping stage")
    client = fetch(downloads["client"]["url"], directory / "client.jar", pin["client_sha1"])
    server = fetch(downloads["server"]["url"], directory / "server.jar", pin["server_sha1"])
    libraries = []
    for library in manifest["libraries"]:
        artifact = library.get("downloads", {}).get("artifact")
        if artifact and allowed(library):
            libraries.append(fetch(artifact["url"], directory / "libraries/client" / safe(artifact["path"]), artifact["sha1"]))
    server_files = bundle(server, directory)
    (directory / "classpath.txt").write_text(":".join(map(str, [client, *libraries])) + "\n")
    (directory / "server-classpath.txt").write_text(":".join(map(str, server_files)) + "\n")
    return manifest, client, libraries


def decompile(java, client, libraries, threads):
    pin = PIN["vineflower"]
    tool = fetch(pin["url"], HOME / "downloads" / Path(pin["url"]).name, pin["sha256"], "sha256")
    destination = HOME / "sources" / "client"
    stamp = destination / ".client-sha1"
    if stamp.is_file() and stamp.read_text().strip() == PIN["minecraft"]["client_sha1"]:
        return
    stage = Path(tempfile.mkdtemp(dir=HOME, prefix="sources-"))
    command = [str(java), "-Xmx6G", "-jar", str(tool), "--folder", "--skip-extra-files=1", "--log-level=warn",
               f"--thread-count={threads}", *(f"--add-external={p}" for p in libraries), str(client), str(stage)]
    print(f"decompiling the client into {destination} (several minutes)", flush=True)
    with open(HOME / "decompile.log", "w") as log:
        subprocess.run(command, check=True, stdout=log, stderr=subprocess.STDOUT)
    shutil.rmtree(destination, ignore_errors=True)
    destination.parent.mkdir(parents=True, exist_ok=True)
    stage.rename(destination)
    stamp.write_text(PIN["minecraft"]["client_sha1"] + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--no-decompile", action="store_true")
    parser.add_argument("--threads", type=int, default=8)
    args = parser.parse_args()
    HOME.mkdir(parents=True, exist_ok=True)
    java = jdk()
    manifest, client, libraries = game()
    (HOME / "env.sh").write_text(
        f"export JAVA_TRUTH={HOME} JAVA_HOME={HOME / 'jdk'} PATH={HOME / 'jdk/bin'}:$PATH\n"
        f"export MC_VERSION={manifest['id']} MC_CLASSPATH=$(cat {HOME / 'versions' / manifest['id'] / 'classpath.txt'})\n")
    if not args.no_decompile:
        decompile(java, client, libraries, args.threads)
    print(f"oracle home ready: {HOME} (Minecraft {manifest['id']}, {len(libraries)} client libraries)")


if __name__ == "__main__":
    main()
