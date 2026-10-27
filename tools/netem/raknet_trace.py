"""How a RakNet peer recovers from loss, read off a netem --capture file (capture.rs).

    py -3 tools/netem/raknet_trace.py CAPTURE [CAPTURE ...]

For each direction it follows reliable messages by their index: a resend is a message that
comes again in a later datagram. A resend whose last copy netem dropped is a recovery, timed
from that copy; one whose last copy got through is spurious (an ACK lost or late). Also counted:
the datagrams sent, lost and dropped, and the ACK and NACK records the other side sent.
"""

import statistics
import sys
from collections import defaultdict

RELIABLE = {2, 3, 4, 6, 7}
SEQUENCED = {1, 4}
ORDERED = {1, 3, 4, 7}


def u24(data, at):
    return data[at] | data[at + 1] << 8 | data[at + 2] << 16


def frames(data):
    """The reliable message indexes of a data datagram's frames."""
    at, indexes = 4, []
    while at + 3 <= len(data):
        flags = data[at]
        reliability, split = flags >> 5, flags & 0x10
        length = (int.from_bytes(data[at + 1:at + 3], "big") + 7) // 8
        at += 3
        if reliability in RELIABLE:
            indexes.append(u24(data, at))
            at += 3
        if reliability in SEQUENCED:
            at += 3
        if reliability in ORDERED:
            at += 4
        if split:
            at += 10
        at += length
    return indexes


def records(data):
    """How many datagram numbers an ACK or NACK covers."""
    count, at, covered = int.from_bytes(data[1:3], "big"), 3, 0
    for _ in range(count):
        if at >= len(data):
            break
        single = data[at]
        start = u24(data, at + 1)
        if single:
            covered, at = covered + 1, at + 4
        else:
            covered, at = covered + u24(data, at + 4) - start + 1, at + 7
    return covered


class Side:
    def __init__(self):
        self.sent = self.lost = self.full = self.bytes = 0
        self.seqs = set()
        self.acks = self.nacks = 0
        self.last = {}  # reliable index -> (time, fate) of its last copy
        self.recoveries = []
        self.spurious = []
        self.first = self.end = None


def read(path):
    sides = defaultdict(Side)
    with open(path) as capture:
        for line in capture:
            parts = line.split()
            if len(parts) < 4:
                continue
            micros, direction, fate, data = int(parts[0]), parts[1], parts[2], bytes.fromhex(parts[3])
            if not data or data[0] & 0x80 == 0:
                continue
            side = sides[direction]
            other = sides["down" if direction == "up" else "up"]
            if data[0] & 0x40:
                other.acks += records(data) if fate == "sent" else 0
                continue
            if data[0] & 0x20:
                other.nacks += records(data) if fate == "sent" else 0
                continue
            seconds = micros / 1e6
            side.first = seconds if side.first is None else side.first
            side.end = seconds
            side.sent += 1
            side.bytes += len(data)
            side.seqs.add(u24(data, 1))
            side.lost += fate == "lost"
            side.full += fate == "full"
            for index in frames(data):
                if index in side.last:
                    when, was = side.last[index]
                    (side.spurious if was == "sent" else side.recoveries).append(seconds - when)
                side.last[index] = (seconds, fate)
    return sides


def summary(name, side):
    def spread(times):
        if not times:
            return "-"
        times = sorted(times)
        return f"median {statistics.median(times) * 1000:.0f} ms, p90 {times[int(len(times) * 0.9)] * 1000:.0f} ms, max {times[-1] * 1000:.0f} ms"

    span = (side.end or 0) - (side.first or 0)
    # Sequence numbers netem never saw: dropped before it, mostly by its own socket's receive buffer.
    unseen = max(side.seqs) - min(side.seqs) + 1 - len(side.seqs) if side.seqs else 0
    return (
        f"  {name}: {side.sent} datagrams ({side.bytes // 1000} kB, {side.bytes // max(side.sent, 1)} B each) "
        f"over {span:.1f} s, {unseen} never reached netem, {side.lost} lost, {side.full} dropped full; "
        f"acked {side.acks}, nacked {side.nacks}\n"
        f"    recoveries {len(side.recoveries)} ({spread(side.recoveries)})\n"
        f"    spurious resends {len(side.spurious)} ({spread(side.spurious)})"
    )


def main(paths):
    for path in paths:
        sides = read(path)
        print(path)
        for name in ("down", "up"):
            if name in sides:
                print(summary(name, sides[name]))


if __name__ == "__main__":
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    main(sys.argv[1:])
