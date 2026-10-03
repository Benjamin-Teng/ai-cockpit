<#
.SYNOPSIS
  安裝檔與 zip 的冒煙測試：真的安裝、啟動、解除安裝，逐項對照 openspec release-distribution 規格。

.DESCRIPTION
  順序見 change release-packaging 的 design D6。會在執行者的帳號下安裝與解除安裝 AI Agent Cockpit、
  建立與刪除捷徑、啟動 cockpit.exe 佔用 127.0.0.1:7770，所以只在用完即丟的環境（GitHub Actions runner）跑：
  沒有 CI 環境變數時拒跑，除非明確加 -AllowOnThisMachine。

  每項檢查印 PASS 或 FAIL（含期望值與實際值）；任一 FAIL 以結束碼 1 結束。

.EXAMPLE
  pwsh -NoProfile -File packaging/smoke-test.ps1 -Setup dist/ai-cockpit-0.1.0-x64-setup.exe `
    -Zip dist/ai-cockpit-0.1.0-x64.zip -Version 0.1.0
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)] [string] $Setup,
    [Parameter(Mandatory)] [string] $Zip,
    [Parameter(Mandatory)] [string] $Version,
    # Inno 的安裝／解除安裝 log、cockpit.exe 輸出、解開的 zip 放這裡（失敗時由 workflow 上傳）。
    [string] $LogDir = (Join-Path ([IO.Path]::GetTempPath()) "ai-cockpit-smoke-$PID"),
    [switch] $AllowOnThisMachine
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if (-not $env:CI -and -not $AllowOnThisMachine) {
    [Console]::Error.WriteLine('smoke-test.ps1 installs and uninstalls AI Agent Cockpit for the current user. ' +
        'Run it only on a throwaway machine (CI is not set); pass -AllowOnThisMachine to override.')
    exit 2
}

$Setup = (Resolve-Path $Setup).Path
$Zip = (Resolve-Path $Zip).Path

$AppName = 'AI Agent Cockpit'
$AppDir = Join-Path $env:LOCALAPPDATA "Programs\$AppName"
$DataDir = Join-Path $env:LOCALAPPDATA 'ai-cockpit'
$UninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\BenjaminTeng.AIAgentCockpit_is1'
$StartMenuLnk = Join-Path ([Environment]::GetFolderPath('Programs')) "$AppName.lnk"
$DesktopLnk = Join-Path ([Environment]::GetFolderPath('Desktop')) "$AppName.lnk"
$AppFiles = @('cockpit.exe', 'cockpit-launch.exe', 'cockpit.example.toml', 'LICENSE.txt')
New-Item -ItemType Directory -Force $LogDir | Out-Null

$script:Failures = 0

function Check([string] $Name, $Expected, $Actual) {
    # 不分大小寫：路徑比對在 Windows 上不分大小寫。
    if ("$Expected" -ieq "$Actual") {
        Write-Host "PASS  $Name"
    } else {
        Write-Host "FAIL  $Name"
        Write-Host "      expected: $Expected"
        Write-Host "      actual:   $Actual"
        $script:Failures++
    }
}

function Step([string] $Title) {
    Write-Host ''
    Write-Host "== $Title"
}

# 執行安裝檔或解除安裝程式（靜默），回傳結束碼；Inno 的 log 留在 $LogDir 供除錯。
# PowerShell 7 的 Start-Process -Wait 會等整個程序樹結束，含解除安裝程式在 TEMP 產生的副本。
function Invoke-Silent([string] $Exe, [string] $LogName, [string[]] $Extra = @()) {
    $log = Join-Path $LogDir "$LogName.log"
    $argList = @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', "/LOG=`"$log`"") + $Extra
    $p = Start-Process -FilePath $Exe -ArgumentList $argList -Wait -PassThru
    Write-Host "      $LogName exit code: $($p.ExitCode) (log: $log)"
    return $p.ExitCode
}

function Invoke-Uninstall([string] $LogName) {
    $cmd = (Get-ItemProperty -Path $UninstallKey -Name UninstallString).UninstallString.Trim('"')
    return (Invoke-Silent $cmd $LogName)
}

# 解除安裝程式結束時移除可能還在收尾（design D6）：輪詢到程式目錄、捷徑、登錄都消失，逾時就交給 Check-Removed 報 FAIL。
function Wait-Removed([int] $TimeoutSec = 60) {
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while ((Get-Date) -lt $deadline) {
        if (-not (Test-Path $AppDir) -and -not (Test-Path $StartMenuLnk) -and -not (Test-Path $DesktopLnk) -and
            (Count-UninstallEntries) -eq 0) { return }
        Start-Sleep -Milliseconds 500
    }
}

# Inno log 裡複製檔案的紀錄（每個 [Files] 項目一段 "-- File entry --"）。
function Count-FileEntries([string] $LogName) {
    $log = Join-Path $LogDir "$LogName.log"
    if (-not (Test-Path $log)) { return -1 }
    return @(Select-String -Path $log -SimpleMatch '-- File entry --').Count
}

function Get-Shortcut([string] $Path) {
    $shell = New-Object -ComObject WScript.Shell
    return $shell.CreateShortcut($Path)
}

function Check-Shortcut([string] $Label, [string] $Path) {
    Check "$Label shortcut exists" $true (Test-Path $Path)
    if (Test-Path $Path) {
        $lnk = Get-Shortcut $Path
        Check "$Label shortcut target" (Join-Path $AppDir 'cockpit-launch.exe') $lnk.TargetPath
        Check "$Label shortcut working directory" $DataDir $lnk.WorkingDirectory
        Check "$Label shortcut arguments" '' $lnk.Arguments
    }
}

function Get-CockpitProcesses {
    return @(Get-Process -Name 'cockpit', 'cockpit-launch' -ErrorAction SilentlyContinue)
}

# 程式目錄各檔的 SHA-256（缺檔記為 missing，不丟例外，讓後續檢查照常報 FAIL）。
function Get-FileHashes {
    $parts = foreach ($f in $AppFiles) {
        $p = Join-Path $AppDir $f
        if (Test-Path $p) { "$f=$((Get-FileHash $p -Algorithm SHA256).Hash)" } else { "$f=missing" }
    }
    return $parts -join ';'
}

function Count-UninstallEntries {
    $roots = @(
        'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall',
        'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall',
        'HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall'
    )
    $n = 0
    foreach ($r in $roots) {
        if (Test-Path $r) {
            $n += @(Get-ChildItem $r | Where-Object { $_.GetValue('DisplayName') -eq $AppName }).Count
        }
    }
    return $n
}

function Check-Removed([string] $Label) {
    Check "$Label removes program directory" $false (Test-Path $AppDir)
    foreach ($f in $AppFiles) {
        Check "$Label removes $f" $false (Test-Path (Join-Path $AppDir $f))
    }
    Check "$Label removes Start menu shortcut" $false (Test-Path $StartMenuLnk)
    Check "$Label removes desktop shortcut" $false (Test-Path $DesktopLnk)
    Check "$Label removes uninstall entry" 0 (Count-UninstallEntries)
    Check "$Label keeps data directory" $true (Test-Path $DataDir -PathType Container)
}

# ---------------------------------------------------------------------------------------------

Step 'Precondition: clean machine'
Check 'program directory absent before install' $false (Test-Path $AppDir)
Check 'no uninstall entry before install' 0 (Count-UninstallEntries)
if ($script:Failures -gt 0) {
    Write-Host 'Not a clean machine; refusing to continue.'
    exit 1
}

Step '1. Install with defaults'
Check 'setup exit code' 0 (Invoke-Silent $Setup 'install-1')
# 對照組：正常安裝的 log 必須有檔案複製紀錄，否則第 4 步「沒有複製紀錄」的判定沒有意義。
Check 'install log records copied files' $true ((Count-FileEntries 'install-1') -gt 0)
foreach ($f in $AppFiles) {
    Check "installs $f" $true (Test-Path (Join-Path $AppDir $f) -PathType Leaf)
}
Check 'data directory exists' $true (Test-Path $DataDir -PathType Container)
Check-Shortcut 'Start menu' $StartMenuLnk
Check-Shortcut 'Desktop' $DesktopLnk
Check 'uninstall entry exists (HKCU, per-user)' $true (Test-Path $UninstallKey)
if (Test-Path $UninstallKey) {
    $entry = Get-ItemProperty $UninstallKey
    Check 'DisplayName' $AppName $entry.DisplayName
    Check 'DisplayVersion' $Version $entry.DisplayVersion
    Check 'InstallLocation' ($AppDir.TrimEnd('\') + '\') $entry.InstallLocation
}
Check 'exactly one uninstall entry' 1 (Count-UninstallEntries)

Step '2. Silent install starts nothing'
Check 'no cockpit process after silent install' 0 @(Get-CockpitProcesses).Count

Step '3. Installed cockpit.exe serves the dashboard (zero config)'
$server = Start-Process -FilePath (Join-Path $AppDir 'cockpit.exe') -WorkingDirectory $DataDir -PassThru `
    -WindowStyle Hidden -RedirectStandardOutput (Join-Path $LogDir 'cockpit.out') `
    -RedirectStandardError (Join-Path $LogDir 'cockpit.err')
$status = 'no response'
$deadline = (Get-Date).AddSeconds(30)
while ((Get-Date) -lt $deadline) {
    try {
        $status = (Invoke-WebRequest -Uri 'http://127.0.0.1:7770/' -UseBasicParsing -TimeoutSec 2).StatusCode
        break
    } catch {
        if ($server.HasExited) { $status = "cockpit.exe exited with $($server.ExitCode)"; break }
        Start-Sleep -Milliseconds 500
    }
}
Check 'GET http://127.0.0.1:7770/' 200 $status
$owner = @(Get-NetTCPConnection -LocalPort 7770 -State Listen -ErrorAction SilentlyContinue |
        Select-Object -ExpandProperty OwningProcess -Unique)
Check 'port 7770 is owned by the installed cockpit.exe' "$($server.Id)" ($owner -join ',')

Step '4. Running Cockpit blocks setup and uninstall'
# Inno 會還原內嵌檔案的時間戳，同一個安裝檔覆寫後修改時間也不變，所以「沒覆寫」以結束碼 7（Preparing to Install
# 判定無法繼續）與 log 中沒有任何檔案複製紀錄判定；雜湊比對只防檔案被刪或換掉。
$before = Get-FileHashes
Check 'setup while running exits with 7 (cannot proceed)' 7 (Invoke-Silent $Setup 'install-while-running')
Check 'setup while running copies no files' 0 (Count-FileEntries 'install-while-running')
Check 'setup while running leaves files in place' $before (Get-FileHashes)
Check 'setup while running leaves cockpit.exe running' $false $server.HasExited
# InitializeUninstall 回 False＝中止，第一階段以 1 結束（Inno 6.7.1 Setup.Uninstall.pas）。
Check 'uninstall while running exits non-zero' $true ((Invoke-Uninstall 'uninstall-while-running') -ne 0)
Check 'uninstall while running leaves files in place' $before (Get-FileHashes)
Check 'uninstall while running leaves Start menu shortcut' $true (Test-Path $StartMenuLnk)
Check 'uninstall while running leaves uninstall entry' 1 (Count-UninstallEntries)
Check 'uninstall while running leaves cockpit.exe running' $false $server.HasExited
Stop-Process -Id $server.Id -Force
$server.WaitForExit()

Step '5. Re-running setup updates in place'
Check 'update exit code' 0 (Invoke-Silent $Setup 'install-update')
Check 'update keeps program directory' ($AppDir.TrimEnd('\') + '\') (Get-ItemProperty $UninstallKey).InstallLocation
Check 'update keeps a single uninstall entry' 1 (Count-UninstallEntries)

Step '6. Uninstall with an empty data directory'
Check 'data directory is empty' 0 @(Get-ChildItem -Force $DataDir).Count
Check 'uninstall exit code' 0 (Invoke-Uninstall 'uninstall-empty-data')
Wait-Removed
Check-Removed 'uninstall'

Step '7. Install without the desktop shortcut'
Check 'setup exit code' 0 (Invoke-Silent $Setup 'install-no-desktop' @('/MERGETASKS=!desktopicon'))
Check 'Start menu shortcut exists' $true (Test-Path $StartMenuLnk)
Check 'desktop shortcut absent' $false (Test-Path $DesktopLnk)
$installed = @{}
foreach ($exe in 'cockpit.exe', 'cockpit-launch.exe') {
    $p = Join-Path $AppDir $exe
    $installed[$exe] = if (Test-Path $p) { (Get-FileHash $p -Algorithm SHA256).Hash } else { 'missing' }
}

Step '8. Uninstall keeps the user''s cockpit.toml'
$toml = Join-Path $DataDir 'cockpit.toml'
Set-Content -Path $toml -Value '# smoke test' -Encoding utf8
Check 'uninstall exit code' 0 (Invoke-Uninstall 'uninstall-with-config')
Wait-Removed
Check-Removed 'uninstall'
Check 'cockpit.toml kept' $true (Test-Path $toml -PathType Leaf)

Step '9. Zip contents'
$unzipped = Join-Path $LogDir 'zip'
Expand-Archive -Path $Zip -DestinationPath $unzipped -Force
$entries = (Get-ChildItem -Force $unzipped | Sort-Object Name | ForEach-Object { $_.Name }) -join ','
Check 'zip root entries' (($AppFiles | Sort-Object) -join ',') $entries
foreach ($exe in 'cockpit.exe', 'cockpit-launch.exe') {
    $zipped = Join-Path $unzipped $exe
    if (Test-Path $zipped) {
        Check "zip $exe matches installed copy" $installed[$exe] (Get-FileHash $zipped -Algorithm SHA256).Hash
    }
}

Write-Host ''
if ($script:Failures -gt 0) {
    Write-Host "$($script:Failures) check(s) failed."
    exit 1
}
Write-Host 'All smoke checks passed.'
