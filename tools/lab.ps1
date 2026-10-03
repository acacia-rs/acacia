# Starts the local Geyser lab: Paper 26.3 (Java :25566) + Geyser Standalone with Boar (Bedrock :19150),
# mirroring a typical Geyser + Boar server. Logs: .testserver/lab/lab.log and .testserver/lab/geyser/geyser.log.
# Stop with: Stop-Process -Id <printed ids>
$dir = Join-Path $PSScriptRoot '..\.testserver\lab'
$java = Join-Path (Get-ChildItem (Join-Path $dir 'jdk') -Directory | Select-Object -First 1).FullName 'bin\java.exe'
$start = {
    param($workdir, $jar, $log, $heap)
    Start-Process -FilePath $java -WorkingDirectory $workdir `
        -ArgumentList "-Xmx$heap", '-jar', $jar, '--nogui' `
        -RedirectStandardOutput (Join-Path $workdir $log) -RedirectStandardError (Join-Path $workdir "$log.err") `
        -NoNewWindow -PassThru | Select-Object Id, @{ n = 'What'; e = { $jar } }
}
& $start $dir 'paper.jar' 'lab.log' '1G'
& $start (Join-Path $dir 'geyser') 'Geyser-Standalone.jar' 'geyser.log' '512M'
