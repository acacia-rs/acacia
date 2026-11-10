"""1 Hz /proc sampler for bench100-profile.sh: per-thread scheduler stats and process memory."""
import os
import sys
import time


def read(path):
    with open(path, errors="replace") as f:
        return f.read()


def status_fields(text, keys):
    found = {}
    for line in text.splitlines():
        key, _, value = line.partition(":")
        if key in keys:
            found[key] = value.split()[0]
    return found


def sample(pid, tasks, proc):
    now = f"{time.time():.3f}"
    for tid in os.listdir(f"/proc/{pid}/task"):
        base = f"/proc/{pid}/task/{tid}"
        try:
            cpu_ns, runq_ns, slices = read(f"{base}/schedstat").split()[:3]
            comm = read(f"{base}/comm").strip().replace(" ", "_")
            s = status_fields(read(f"{base}/status"), {"State", "voluntary_ctxt_switches", "nonvoluntary_ctxt_switches"})
        except OSError:
            continue
        tasks.write(f"{now}\t{tid}\t{comm}\t{cpu_ns}\t{runq_ns}\t{slices}\t{s.get('voluntary_ctxt_switches', 0)}\t{s.get('nonvoluntary_ctxt_switches', 0)}\t{s.get('State', '?')}\n")
    roll = status_fields(read(f"/proc/{pid}/smaps_rollup"), {"Rss", "Anonymous", "Shared_Clean", "Private_Clean"})
    proc.write(f"{now}\t{roll.get('Rss', 0)}\t{roll.get('Anonymous', 0)}\n")


def main(pid, out):
    with open(f"{out}/tasks.tsv", "w") as tasks, open(f"{out}/proc.tsv", "w") as proc:
        while os.path.isdir(f"/proc/{pid}"):
            try:
                sample(pid, tasks, proc)
            except OSError:
                break
            tasks.flush()
            proc.flush()
            time.sleep(1)


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
