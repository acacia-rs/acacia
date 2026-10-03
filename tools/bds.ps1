# Starts the local Bedrock Dedicated Server test instance (RakNet, port 19140).
# -Strict sets the movement-fuzzing profile (docs/testing.md "Movement physics"): strict server-authoritative movement
# correcting every 0.0001 off, no rewind delay, offline mode for named fuzz bots. Without it BDS's default movement
# settings are restored.
# Output goes to .testserver/bds.log. Stop it with: Stop-Process -Name bedrock_server
param([switch]$Strict)

$dir = Join-Path $PSScriptRoot '..\.testserver\bds'
$log = Join-Path $PSScriptRoot '..\.testserver\bds.log'
$props = Join-Path $dir 'server.properties'

$settings = if ($Strict) {
    @{ 'server-authoritative-movement-strict' = 'true'; 'player-position-acceptance-threshold' = '0.0001'
       'player-rewind-min-correction-delay-ticks' = '0'; 'online-mode' = 'false' }
} else {
    @{ 'server-authoritative-movement-strict' = 'false'; 'player-position-acceptance-threshold' = '0.5'
       'player-rewind-min-correction-delay-ticks' = $null }
}
$lines = [System.Collections.Generic.List[string]](Get-Content $props)
foreach ($key in $settings.Keys) {
    $at = -1
    for ($i = 0; $i -lt $lines.Count; $i++) { if ($lines[$i].StartsWith("$key=")) { $at = $i; break } }
    $value = $settings[$key]
    if ($null -eq $value) { if ($at -ge 0) { $lines.RemoveAt($at) } }
    elseif ($at -ge 0) { $lines[$at] = "$key=$value" }
    else { $lines.Add("$key=$value") }
}
Set-Content -Path $props -Value $lines

Start-Process -FilePath (Join-Path $dir 'bedrock_server.exe') -WorkingDirectory $dir `
    -RedirectStandardOutput $log -NoNewWindow -PassThru | Select-Object Id
