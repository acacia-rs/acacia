# Builds .testserver/bds-packs: a copy of the RakNet test BDS on port 19150 whose world requires
# tools/packtest-pack, padded with 3 MB of random bytes so a download spans several chunks.
# Start it with: tools/bds.ps1 -Name bds-packs
$root = Join-Path $PSScriptRoot '..\.testserver'
$dst = Join-Path $root 'bds-packs'
if (-not (Test-Path $dst)) {
    robocopy (Join-Path $root 'bds') $dst /E /XD worlds /NFL /NDL /NJH /NJS | Out-Null
    # robocopy exits 1-7 on success.
    if ($LASTEXITCODE -ge 8) { throw "robocopy failed with $LASTEXITCODE" }
    $global:LASTEXITCODE = 0
}

$props = Join-Path $dst 'server.properties'
$set = @{ 'server-port' = '19150'; 'server-portv6' = '19151'; 'level-name' = 'packs'; 'texturepack-required' = 'true' }
$lines = Get-Content $props | ForEach-Object {
    $key = ($_ -split '=', 2)[0]
    if ($set.ContainsKey($key)) { "$key=$($set[$key])" } else { $_ }
}
Set-Content $props $lines

$pack = Join-Path $dst 'resource_packs\packtest'
New-Item -ItemType Directory -Force $pack | Out-Null
Copy-Item (Join-Path $PSScriptRoot 'packtest-pack\manifest.json') $pack -Force
# Kept once made: new bytes would change the pack hash that bot caches key on.
$padding = Join-Path $pack 'padding.bin'
if (-not (Test-Path $padding)) {
    $bytes = New-Object byte[] (3MB)
    [Security.Cryptography.RandomNumberGenerator]::Fill($bytes)
    [IO.File]::WriteAllBytes($padding, $bytes)
}

$world = Join-Path $dst 'worlds\packs'
New-Item -ItemType Directory -Force $world | Out-Null
Set-Content (Join-Path $world 'world_resource_packs.json') '[{ "pack_id": "8fbd6e02-fb25-4e1d-adc2-776df9dfb30b", "version": [1, 0, 0] }]'
Write-Output "bds-packs ready on port 19150"
