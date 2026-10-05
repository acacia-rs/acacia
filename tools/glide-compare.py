"""Line up the bot's per-tick glide (auth input trace) with the pack's server-side glide log.
usage: py -3 tools/glide-compare.py <bot.log> <bds.log>   (see tools/actiontest-pack/README.md)
BDS sometimes takes two inputs in one server tick, so rows can slip by one; read the trend."""
import re
import sys

ansi = re.compile(r"\x1b\[[0-9;]*m")
bot = []
for line in open(sys.argv[1], encoding="utf-8", errors="replace"):
    line = ansi.sub("", line)
    m = re.search(r"auth input tick=(\d+).*?pitch=([-\d.e]+) pos=\[([^\]]+)\] delta=\[([^\]]+)\].*flags=\[([^\]]*)\]", line)
    if m:
        bot.append((int(m[1]), float(m[2]), [float(x) for x in m[3].split(",")], [float(x) for x in m[4].split(",")], m[5]))
srv = []
for line in open(sys.argv[2], encoding="utf-8", errors="replace"):
    m = re.search(r"ACTIONTEST glide (\d+) ([-\d.,]+) v ([-\d.,]+) rot ([-\d.]+),", line)
    if m:
        srv.append((int(m[1]), [float(x) for x in m[2].split(",")], [float(x) for x in m[3].split(",")], float(m[4])))

# Align: the server tick whose position is closest to a bot position after the glide start.
start = next(i for i, b in enumerate(bot) if "StartGliding" in b[4])
best = min(((abs(s[1][0] - b[2][0]) + abs(s[1][1] - b[2][1]), bi, si)
            for bi, b in enumerate(bot[start:start + 15], start) for si, s in enumerate(srv[:15])))
_, bi, si = best
print(f"aligned bot tick {bot[bi][0]} with server tick {srv[si][0]} (err {best[0]:.4f})")
print("tick  pitch(bot/srv)   bot pos               srv pos               dpos            bot delta.y  srv v.y   flags")
for k in range(0, min(len(bot) - bi, len(srv) - si)):
    b, s = bot[bi + k], srv[si + k]
    d = [b[2][i] - s[1][i] for i in range(3)]
    flags = ",".join(f for f in b[4].split(", ") if f in ("StartGliding", "StopGliding", "JumpDown", "VerticalCollision"))
    print(f"{b[0]:4d} {b[1]:6.1f}/{s[3]:6.1f}  {b[2][0]:8.3f},{b[2][1]:8.3f}  {s[1][0]:8.3f},{s[1][1]:8.3f}  "
          f"{d[0]:+7.3f},{d[1]:+7.3f}  {b[3][1]:+8.4f}  {s[2][1]:+8.4f}  {flags}")
