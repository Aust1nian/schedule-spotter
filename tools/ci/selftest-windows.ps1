# Installs the app silently as the current user, confirms no admin rights
# were needed, runs --self-test, and fails if the app or any of its child
# processes (the WebView2 runtime) opened a network socket while it ran.
# Usage: selftest-windows.ps1 -Installer <setup.exe> [-Out selftest.json]
param(
  [Parameter(Mandatory = $true)][string]$Installer,
  [string]$Out = (Join-Path (Get-Location) "selftest.json")
)
$ErrorActionPreference = "Stop"

$setup = Start-Process -FilePath $Installer -ArgumentList "/S" -Wait -PassThru
if ($setup.ExitCode -ne 0) { throw "installer exited with $($setup.ExitCode)" }

$exe = Get-ChildItem -Path $env:LOCALAPPDATA -Recurse -Depth 3 -Filter *.exe -ErrorAction SilentlyContinue |
  Where-Object { $_.FullName -match "Schedule Spotter" -and $_.Name -notmatch "^uninstall" } |
  Select-Object -First 1
if (-not $exe) { throw "installed app not found under $env:LOCALAPPDATA" }
Write-Host "installed: $($exe.FullName)"

function Get-Descendants([int]$ParentId) {
  $kids = Get-CimInstance Win32_Process -Filter "ParentProcessId=$ParentId" -ErrorAction SilentlyContinue
  foreach ($kid in $kids) {
    $kid.ProcessId
    Get-Descendants $kid.ProcessId
  }
}

$proc = Start-Process -FilePath $exe.FullName -ArgumentList "--self-test", "--json-out", "`"$Out`"" -PassThru
$null = $proc.Handle  # keeps ExitCode readable after exit
$sockets = @()
while (-not $proc.HasExited) {
  foreach ($id in @($proc.Id) + @(Get-Descendants $proc.Id)) {
    $sockets += @(Get-NetTCPConnection -OwningProcess $id -ErrorAction SilentlyContinue |
      ForEach-Object { "TCP pid=$id $($_.LocalAddress):$($_.LocalPort) -> $($_.RemoteAddress):$($_.RemotePort) $($_.State)" })
    $sockets += @(Get-NetUDPEndpoint -OwningProcess $id -ErrorAction SilentlyContinue |
      ForEach-Object { "UDP pid=$id $($_.LocalAddress):$($_.LocalPort)" })
  }
  Start-Sleep -Milliseconds 200
}
$proc.WaitForExit()

if (Test-Path $Out) { Get-Content $Out } else { Write-Host "no self-test output" }
if ($sockets.Count -gt 0) {
  Write-Host "FAIL: the app opened network sockets:"
  $sockets | Sort-Object -Unique | ForEach-Object { Write-Host $_ }
  exit 1
}
Write-Host "no sockets opened"
exit $proc.ExitCode
