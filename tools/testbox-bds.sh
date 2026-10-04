#!/usr/bin/env bash
# Linux BDS for live action tests on testbox (~/work/bds-actions, RakNet :19170, offline, flat world,
# tools/actiontest-pack enabled). Run from this machine; every command goes over ssh.
# usage: tools/testbox-bds.sh install|pack|start|stop|restart|reset|log [n]|cmd <console command>
#   a second instance: BDS_DIR=work/bds-actions2 BDS_PORT=19172 tools/testbox-bds.sh ...
#   install  download BDS, write server.properties, install the pack (idempotent)
#   pack     re-sync tools/actiontest-pack (then restart)
#   reset    delete the world (a fresh scene on the next start)
set -uo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
dir=${BDS_DIR:-work/bds-actions}
port=${BDS_PORT:-19170}
version=1.26.52.3
url=https://www.minecraft.net/bedrockdedicatedserver/bin-linux/bedrock-server-$version.zip
pack_uuid=$(sed -n 's/.*"uuid": "\(.*\)",/\1/p' "$root/tools/actiontest-pack/manifest.json" | head -1)
opts="-o ConnectTimeout=20 -o ServerAliveInterval=10"
retry() { for i in 1 2 3 4 5; do "$@" && return 0; echo "retry $i: $1" >&2; sleep 3; done; return 1; }
remote() { retry ssh $opts testbox "$1"; }
ctl() { ssh $opts testbox "cd $dir && bash -s $1" <"$root/tools/bds-ctl.sh"; }

props="server-name=actiontest
gamemode=survival
difficulty=easy
level-name=actions
level-type=FLAT
level-seed=1
online-mode=false
allow-list=false
allow-cheats=true
default-player-permission-level=operator
server-port=$port
server-portv6=$((port + 1))
transport=raknet
enable-lan-visibility=false
view-distance=8
tick-distance=4
max-players=10
content-log-console-output-enabled=true
content-log-file-enabled=false
emit-server-telemetry=false"

install_pack() {
  remote "rm -rf $dir/development_behavior_packs/actiontest; mkdir -p $dir/development_behavior_packs/actiontest $dir/worlds/actions" || exit 1
  tar czf - -C "$root/tools/actiontest-pack" . | retry ssh $opts testbox "tar xzf - -C $dir/development_behavior_packs/actiontest" || exit 1
  remote "echo '[{ \"pack_id\": \"$pack_uuid\", \"version\": [1, 0, 0] }]' > $dir/worlds/actions/world_behavior_packs.json"
}

case "${1:-}" in
  install)
    remote "mkdir -p $dir && cd $dir || exit; test -x bedrock_server || { curl -fsSL -A 'Mozilla/5.0' -o bds.zip '$url' && unzip -qo bds.zip && rm bds.zip; }" || exit 1
    # Override only our keys; the rest of BDS's default server.properties stays.
    printf '%s\n' "$props" | retry ssh $opts testbox "cd $dir || exit; cat > props.new; awk -F= 'NR==FNR{k[\$1]=1; print; next} !(\$1 in k)' props.new server.properties > p.tmp && mv p.tmp server.properties; rm props.new" || exit 1
    install_pack ;;
  pack) install_pack ;;
  start|stop|reset) retry ctl "$1" ;;
  restart) "$0" stop; "$0" start ;;
  log) remote "tail -n ${2:-60} $dir/bds.log" ;;
  cmd) shift; remote "echo $(printf '%q ' "$@") > $dir/console" ;;
  *) sed -n '2,7p' "$0"; exit 2 ;;
esac
