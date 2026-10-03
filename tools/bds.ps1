# Starts the local Bedrock Dedicated Server test instance (offline mode, RakNet, port 19140).
# Output goes to .testserver/bds.log. Stop it with: Stop-Process -Name bedrock_server
$dir = Join-Path $PSScriptRoot '..\.testserver\bds'
$log = Join-Path $PSScriptRoot '..\.testserver\bds.log'
Start-Process -FilePath (Join-Path $dir 'bedrock_server.exe') -WorkingDirectory $dir `
    -RedirectStandardOutput $log -NoNewWindow -PassThru | Select-Object Id
