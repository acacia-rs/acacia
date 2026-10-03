# Starts a local Bedrock Dedicated Server test instance from .testserver/<Name> (default `bds`:
# offline, RakNet, port 19140). Output goes to .testserver/<Name>.log.
# Stop it with: Stop-Process -Name bedrock_server
param([string]$Name = 'bds')
$dir = Join-Path $PSScriptRoot "..\.testserver\$Name"
$log = Join-Path $PSScriptRoot "..\.testserver\$Name.log"
Start-Process -FilePath (Join-Path $dir 'bedrock_server.exe') -WorkingDirectory $dir `
    -RedirectStandardOutput $log -NoNewWindow -PassThru | Select-Object Id
