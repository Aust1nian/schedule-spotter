# Install test (spec: "Install tests, every release candidate"): marks the
# installer as downloaded from the internet (Mark of the Web), checks its
# signature and that it asks for no admin rights, installs it silently as a
# separate standard user, runs --self-test from the installed location while
# watching for network sockets, uninstalls it as that user, and checks
# nothing is left behind.
#
# Usage: install-test-windows.ps1 -Installer <setup.exe> -Out <out.json>
#          [-Signed] [-Variant <V>]
#   -Signed  the installer must carry a valid signature. Without it, the
#            signature check is reported as skipped: Windows builds ship
#            unsigned by Austin's decision (DECISIONS.md, 2026-10-11).
param(
  [Parameter(Mandatory = $true)][string]$Installer,
  [Parameter(Mandatory = $true)][string]$Out,
  [switch]$Signed,
  [string]$Variant = ""
)
$ErrorActionPreference = "Stop"
$checks = [System.Collections.Generic.List[object]]::new()
$failed = $false
function Check([string]$Name, [string]$Status, [string]$Detail) {
  $checks.Add([ordered]@{ check = $Name; status = $Status; detail = $Detail })
  Write-Host "[$Status] ${Name}: $Detail"
  if ($Status -eq "fail") { $script:failed = $true }
}

# A folder the standard user can read, holding the installer and the
# script that runs as that user.
$work = "C:\spotter-install-test"
Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory $work | Out-Null
$setup = Join-Path $work (Split-Path $Installer -Leaf)
Copy-Item $Installer $setup
icacls $work /grant "*S-1-5-32-545:(OI)(CI)M" | Out-Null  # BUILTIN\Users

# 1. Downloaded from the internet: zone 3 is the Internet zone.
Set-Content -Path $setup -Stream Zone.Identifier -Value "[ZoneTransfer]`r`nZoneId=3`r`nHostUrl=https://download.invalid/$(Split-Path $Installer -Leaf)"
$zone = (Get-Content -Path $setup -Stream Zone.Identifier) -join " "
Check "mark-of-the-web" $(if ($zone -match "ZoneId=3") { "pass" } else { "fail" }) $zone
Check "installer-size" "info" "$((Get-Item $setup).Length) bytes"

# 2. Signature.
$sig = Get-AuthenticodeSignature $setup
if ($Signed) {
  Check "signature" $(if ($sig.Status -eq "Valid") { "pass" } else { "fail" }) "$($sig.Status): $($sig.SignerCertificate.Subject)"
} else {
  Check "signature" "skip" "$($sig.Status): unsigned by decision (DECISIONS.md, 2026-10-11)"
}

# 3. The installer's manifest asks for no elevation.
$stream = [System.IO.File]::OpenRead($setup)
$head = New-Object byte[] ([int][Math]::Min([long]4MB, $stream.Length))  # installers can pass 2 GiB
[void]$stream.Read($head, 0, $head.Length)
$stream.Close()
$manifest = [System.Text.Encoding]::ASCII.GetString($head)
if ($manifest -match 'requestedExecutionLevel\s+level="(\w+)"') {
  $level = $Matches[1]
  Check "execution-level" $(if ($level -eq "asInvoker") { "pass" } else { "fail" }) "requestedExecutionLevel=$level"
} else {
  Check "execution-level" "fail" "no requestedExecutionLevel in the installer's manifest"
}

# 4. A standard user, allowed to run a scheduled task (log on as a batch job).
$user = "ownertest"  # nothing in the name may match the leftover check
$password = "Sp0t-" + [Guid]::NewGuid().ToString("N")
Get-LocalUser $user -ErrorAction SilentlyContinue | Remove-LocalUser
$secure = ConvertTo-SecureString $password -AsPlainText -Force
$account = New-LocalUser $user -Password $secure -PasswordNeverExpires -AccountNeverExpires
Add-LocalGroupMember -SID "S-1-5-32-545" -Member $user
$isAdmin = [bool](Get-LocalGroupMember -SID "S-1-5-32-544" | Where-Object { $_.SID -eq $account.SID })
Check "standard-user" $(if ($isAdmin) { "fail" } else { "pass" }) "$user, not in Administrators"
$cfg = Join-Path $work "rights.inf"
secedit /export /cfg $cfg /areas USER_RIGHTS | Out-Null
$lines = Get-Content $cfg
if ($lines -match "^SeBatchLogonRight") {
  $lines = $lines -replace "^(SeBatchLogonRight = .*)$", "`$1,*$($account.SID)"
} else {
  $lines = $lines -replace "^\[Privilege Rights\]$", "[Privilege Rights]`r`nSeBatchLogonRight = *$($account.SID)"
}
$lines | Set-Content $cfg -Encoding Unicode
secedit /configure /db (Join-Path $work "rights.sdb") /cfg $cfg /areas USER_RIGHTS | Out-Null

# What runs as the standard user: one phase per scheduled-task run. It
# records what it sees in <phase>.json, since that user's registry hive is
# only loaded while it runs.
$inner = Join-Path $work "as-user.ps1"
@'
param([string]$Phase, [string]$Setup, [string]$Work)
$ErrorActionPreference = "Continue"
Start-Transcript -Path (Join-Path $Work "$Phase.log") -Force | Out-Null
$r = [ordered]@{ phase = $Phase; user = [Environment]::UserName }
# Started directly (CreateProcess), not through the shell: the shell would
# show the "publisher could not be verified" prompt for a file with a Mark
# of the Web, and a scheduled task has no desktop to click it on. An owner
# sees SmartScreen there instead (recovery card).
function Run([string]$File, [string]$Arguments, [int]$Minutes) {
  $psi = New-Object Diagnostics.ProcessStartInfo $File, $Arguments
  $psi.UseShellExecute = $false
  $p = [Diagnostics.Process]::Start($psi)
  if (-not $p.WaitForExit($Minutes * 60000)) {
    Write-Host "$File $Arguments still running after $Minutes min; stopping it"
    $p.Kill()
    return "timeout"
  }
  return $p.ExitCode
}
$r.elevated = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
function Snapshot {
  $roots = @($env:LOCALAPPDATA, $env:APPDATA, [Environment]::GetFolderPath("Desktop"))
  $files = foreach ($root in $roots) {
    Get-ChildItem $root -Recurse -Force -ErrorAction SilentlyContinue | ForEach-Object { $_.FullName }
  }
  $keys = Get-ChildItem "HKCU:\Software" -Recurse -ErrorAction SilentlyContinue | ForEach-Object { $_.Name }
  @($files) + @($keys)
}
$uninstKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Schedule Spotter"
$installDir = Join-Path $env:LOCALAPPDATA "Schedule Spotter"
if ($Phase -eq "install") {
  Snapshot | Set-Content (Join-Path $Work "before.txt") -Encoding UTF8
  $t = [Diagnostics.Stopwatch]::StartNew()
  Write-Host "installing"
  $r.install_exit = Run $Setup "/S" 12
  Write-Host "installer exit $($r.install_exit)"
  $r.install_seconds = [Math]::Round($t.Elapsed.TotalSeconds, 1)
  $r.install_dir = $installDir
  $r.files = @(Get-ChildItem $installDir -Recurse -File -ErrorAction SilentlyContinue | ForEach-Object { "$($_.FullName.Substring($installDir.Length + 1)) $($_.Length)" })
  $r.uninstall_key = Test-Path $uninstKey
  $r.start_menu = @(Get-ChildItem (Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs") -Filter "Schedule Spotter*" -ErrorAction SilentlyContinue | ForEach-Object { $_.Name })
} else {
  $uninstaller = Join-Path $installDir "uninstall.exe"
  $r.uninstaller_found = Test-Path $uninstaller
  if ($r.uninstaller_found) {
    Write-Host "uninstalling"
    $r.uninstall_exit = Run $uninstaller "/S" 5
    # The uninstaller hands over to a copy of itself in %TEMP%; wait for it.
    $deadline = (Get-Date).AddSeconds(180)
    while ((Test-Path $installDir) -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 500 }
    Start-Sleep -Seconds 3
  }
  $before = Get-Content (Join-Path $Work "before.txt") -Encoding UTF8
  $after = Snapshot
  $r.new_paths = @($after | Where-Object { $before -notcontains $_ })
  $r.install_dir_left = Test-Path $installDir
  $r.uninstall_key_left = Test-Path $uninstKey
}
$r | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $Work "$Phase.json") -Encoding UTF8
Stop-Transcript | Out-Null
'@ | Set-Content $inner -Encoding UTF8

function Run-AsUser([string]$Phase) {
  $name = "spotter-$Phase"
  $action = New-ScheduledTaskAction -Execute "powershell.exe" `
    -Argument "-NoProfile -ExecutionPolicy Bypass -File `"$inner`" -Phase $Phase -Setup `"$setup`" -Work `"$work`""
  $settings = New-ScheduledTaskSettingsSet -ExecutionTimeLimit (New-TimeSpan -Minutes 20)
  Register-ScheduledTask -TaskName $name -Action $action -Settings $settings -User $user -Password $password -RunLevel Limited -Force | Out-Null
  Start-ScheduledTask -TaskName $name
  $deadline = (Get-Date).AddMinutes(20)
  do {
    Start-Sleep -Seconds 2
    $state = (Get-ScheduledTask -TaskName $name).State
  } while ($state -eq "Running" -and (Get-Date) -lt $deadline)
  $info = Get-ScheduledTaskInfo -TaskName $name
  Unregister-ScheduledTask -TaskName $name -Confirm:$false
  $json = Join-Path $work "$Phase.json"
  if (-not (Test-Path $json)) {
    Write-Host "--- $Phase log ---"
    Get-Content (Join-Path $work "$Phase.log") -ErrorAction SilentlyContinue
    Get-Process -IncludeUserName -ErrorAction SilentlyContinue | Where-Object { $_.UserName -like "*$user" } |
      ForEach-Object { Write-Host "still running as ${user}: $($_.Name) ($($_.Id))" }
    throw "the $Phase phase left no result (task result $($info.LastTaskResult))"
  }
  Get-Content $json -Raw | ConvertFrom-Json
}

# 5. Install as the standard user.
$installed = Run-AsUser "install"
Check "installed-as" $(if ($installed.elevated) { "fail" } else { "pass" }) "$($installed.user), elevated=$($installed.elevated)"
Check "silent-install" $(if ($installed.install_exit -eq 0) { "pass" } else { "fail" }) "exit $($installed.install_exit) after $($installed.install_seconds) s"
$exe = Get-ChildItem $installed.install_dir -Filter *.exe -ErrorAction SilentlyContinue |
  Where-Object { $_.Name -notmatch "^uninstall" } | Select-Object -First 1
Check "per-user-location" $(if ($exe -and $installed.install_dir -like "C:\Users\$user\AppData\Local\*") { "pass" } else { "fail" }) "$($installed.install_dir)"
Check "uninstall-entry" $(if ($installed.uninstall_key) { "pass" } else { "fail" }) "HKCU uninstall key present: $($installed.uninstall_key)"
Check "start-menu" $(if ($installed.start_menu.Count -gt 0) { "pass" } else { "fail" }) "$($installed.start_menu -join ', ')"
Check "licenses" $(if ($installed.files -match "^Licenses\\README.txt") { "pass" } else { "fail" }) "Licenses folder installed"
if ($Variant -and $Variant -ne "off") {
  $model = $installed.files -match "^model\\.*\.gguf "
  Check "model-installed" $(if ($model) { "pass" } else { "fail" }) "$($model -join ', ')"
}
Check "installed-files" "info" "$($installed.files -join '; ')"

# 6. Self-test from the installed location, watching the app and its
#    WebView2 child processes for sockets.
function Get-Descendants([int]$ParentId) {
  foreach ($kid in Get-CimInstance Win32_Process -Filter "ParentProcessId=$ParentId" -ErrorAction SilentlyContinue) {
    $kid.ProcessId
    Get-Descendants $kid.ProcessId
  }
}
$selftestJson = Join-Path $work "selftest.json"
$proc = Start-Process -FilePath $exe.FullName -ArgumentList "--self-test", "--linger", "60", "--json-out", "`"$selftestJson`"" -PassThru
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
$selftest = $null
if (Test-Path $selftestJson) { $selftest = Get-Content $selftestJson -Raw | ConvertFrom-Json }
Check "self-test" $(if ($proc.ExitCode -eq 0) { "pass" } else { "fail" }) "exit $($proc.ExitCode)"
if ($sockets.Count -gt 0) {
  Check "no-sockets" "fail" (($sockets | Sort-Object -Unique) -join "; ")
} else {
  Check "no-sockets" "pass" "no socket opened during the self-test"
}
# WebView2's processes can hold the scratch folder past the app's exit; a
# hidden copy of the app removes it once they let go (scratch.rs).
$deadline = (Get-Date).AddSeconds(45)
while ((Get-ChildItem $env:TEMP -Filter "schedule-spotter-session-*" -ErrorAction SilentlyContinue) -and (Get-Date) -lt $deadline) {
  Start-Sleep -Milliseconds 500
}
$appData = @("$env:LOCALAPPDATA\com.tightline.schedulespotter", "$env:APPDATA\com.tightline.schedulespotter") | Where-Object { Test-Path $_ }
$scratch = @(Get-ChildItem $env:TEMP -Filter "schedule-spotter-session-*" -ErrorAction SilentlyContinue | ForEach-Object { $_.Name })
Check "writes-nothing" $(if ($appData.Count -eq 0 -and $scratch.Count -eq 0) { "pass" } else { "fail" }) "app data: $($appData -join ', '); scratch left: $($scratch -join ', ')"

# 7. Uninstall as the same user; nothing of the app may be left.
$removed = Run-AsUser "uninstall"
Check "silent-uninstall" $(if ($removed.uninstaller_found -and $removed.uninstall_exit -eq 0) { "pass" } else { "fail" }) "exit $($removed.uninstall_exit)"
Check "install-dir-removed" $(if (-not $removed.install_dir_left) { "pass" } else { "fail" }) "left: $($removed.install_dir_left)"
Check "uninstall-entry-removed" $(if (-not $removed.uninstall_key_left) { "pass" } else { "fail" }) "left: $($removed.uninstall_key_left)"
# Matched without the profile folder's own path. NSIS runs its uninstaller
# from a copy in %TEMP% (~nsu*.tmp\Un.exe), which it can't delete while it
# runs; that one is NSIS's, reported below with the rest.
$profileRoot = "C:\Users\$user"
$ours = @($removed.new_paths | Where-Object { $_.Replace($profileRoot, "") -match "spotter|tight ?line|EBWebView" })
Check "nothing-left" $(if ($ours.Count -eq 0) { "pass" } else { "fail" }) $(if ($ours.Count) { $ours -join "; " } else { "nothing of the app's in the user's profile or registry" })
Check "other-new-paths" "info" "$(@($removed.new_paths).Count) other new paths (Windows and PowerShell's own): $((@($removed.new_paths) | Select-Object -First 8) -join '; ')"

Remove-LocalUser $user
[ordered]@{
  install_test = $(if ($failed) { "fail" } else { "pass" })
  checks = $checks
  self_test = $selftest
} | ConvertTo-Json -Depth 6 | Set-Content $Out -Encoding UTF8
if ($failed) { exit 1 }
exit 0
