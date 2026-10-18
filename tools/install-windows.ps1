# Run from the extracted DXM-088 Windows release. Installs for the current user.
$ErrorActionPreference = 'Stop'
$taskVst3 = Join-Path $env:LOCALAPPDATA 'Programs\Common\VST3'
$taskClap = Join-Path $env:LOCALAPPDATA 'Programs\Common\CLAP'
New-Item -ItemType Directory -Force $taskVst3, $taskClap | Out-Null
foreach ($taskItem in @(
    @{ Name = 'DXM-088.vst3'; Destination = $taskVst3 },
    @{ Name = 'DXM-088.clap'; Destination = $taskClap },
    @{ Name = 'DXM-088.resources'; Destination = $taskClap }
)) {
    $taskSource = Join-Path $PSScriptRoot $taskItem.Name
    if (!(Test-Path $taskSource)) { throw "Missing release file: $taskSource" }
    $taskDestination = Join-Path $taskItem.Destination $taskItem.Name
    # Copy directory contents into an existing bundle without nesting the bundle.
    if (Test-Path $taskSource -PathType Container) {
        New-Item -ItemType Directory -Force $taskDestination | Out-Null
        Copy-Item (Join-Path $taskSource '*') $taskDestination -Recurse -Force
    } else {
        Copy-Item $taskSource $taskDestination -Force
    }
}
Write-Host "DXM-088 installed. Rescan plug-ins in your DAW."
Write-Host "VST3: $taskVst3"
Write-Host "CLAP: $taskClap"
Write-Host 'Keep DXM-088.resources beside DXM-088.clap.'
