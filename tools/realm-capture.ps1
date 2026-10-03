# Records a vanilla Realm join: HTTPS/WebSocket of Minecraft.Windows.exe (mitmproxy local mode) and all
# UDP (pktmon, for STUN/TURN). Run from an elevated PowerShell, join the realm, then press Enter.
# Output (git-ignored; holds Xbox/MCTokens, never share): .testserver/realm-capture/<stamp>/
# One-time: trust the mitmproxy CA for your user, and remove it afterwards:
#   certutil -user -addstore Root $HOME\.mitmproxy\mitmproxy-ca-cert.cer
#   certutil -user -delstore Root mitmproxy
# The game's WebSocket client rejects the mitmproxy CA (its HTTP client doesn't), so its WebSocket
# hosts (signaling, Xbox RTA) pass through untouched; every other HTTPS call is still recorded.
param([string]$Process = "Minecraft.Windows.exe", [string]$IgnoreHosts = "^(signaling-tm-|rta\.xboxlive\.com)")

$root = Split-Path $PSScriptRoot -Parent
$out = Join-Path $root ".testserver\realm-capture\$(Get-Date -Format yyyyMMdd-HHmmss)"
New-Item -ItemType Directory -Force $out | Out-Null
$mitmdump = Join-Path $env:APPDATA "Python\Python313\Scripts\mitmdump.exe"

# Windows PowerShell treats native stderr as an error; pktmon writes there when nothing is running.
cmd /c "pktmon stop >nul 2>&1"
cmd /c "pktmon filter remove >nul 2>&1"
cmd /c "pktmon filter add udp -t UDP >nul 2>&1"
pktmon start --capture --pkt-size 0 --file-name "$out\udp.etl" | Out-Null
if ($LASTEXITCODE -ne 0) { Write-Error "pktmon start failed (run as administrator)"; exit 1 }

# Unbuffered, or the log loses its tail (the TLS failures) when the process is stopped.
$env:PYTHONUNBUFFERED = "1"
$mitm = Start-Process $mitmdump -PassThru -NoNewWindow -RedirectStandardOutput "$out\mitm.log" -RedirectStandardError "$out\mitm.err" `
    -ArgumentList "--mode", "local:$Process", "-w", "`"$out\https.flow`"", "--set", "flow_detail=2", "--ignore-hosts", "`"$IgnoreHosts`""
Write-Host "Capturing to $out"
Write-Host "Now join the realm in Minecraft, play ~30 s, leave, then press Enter here."
Read-Host | Out-Null

Stop-Process -Id $mitm.Id
cmd /c "pktmon stop >nul 2>&1"
cmd /c "pktmon etl2pcap `"$out\udp.etl`" --out `"$out\udp.pcapng`" >nul 2>&1"
cmd /c "pktmon filter remove >nul 2>&1"
Write-Host "Done: $out"
