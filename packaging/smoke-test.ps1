<#
.SYNOPSIS
  安裝檔與 zip 的冒煙測試：真的安裝、啟動、解除安裝，逐項對照 openspec release-distribution 規格。

.DESCRIPTION
  順序見 change release-packaging 的 design D6。會在執行者的帳號下安裝與解除安裝 AI Agent Cockpit、
  建立與刪除捷徑、啟動 cockpit.exe 佔用 127.0.0.1:7770，所以只在用完即丟的環境（GitHub Actions runner）跑：
  沒有 CI 環境變數時拒跑，除非明確加 -AllowOnThisMachine。
  更新模式的兩個情境（5a、5b）見 change auto-update 的 design D10。

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

# 更新模式會由安裝檔重新啟動 cockpit-launch.exe，它繼承本腳本的環境變數（design D10）：
# - COCKPIT_BROWSER：假瀏覽器，不在 runner 上開真的 Chrome／Edge。選 whoami.exe：每台 Windows 都在 System32，
#   收到不認得的 --app=<網址> 引數時印錯誤訊息並以 1 立即結束，不讀標準輸入、不開視窗（啟動器不等瀏覽器、不看結束碼）。
# - COCKPIT_LAUNCH_DIALOG_FILE：啟動器的訊息改寫入這個檔、不跳框；結束時有內容即 FAIL。
# - COCKPIT_NO_UPDATE_CHECK：重新啟動的啟動器不對外查詢更新。
$FakeBrowser = Join-Path $env:SystemRoot 'System32\whoami.exe'
$DialogFile = Join-Path $LogDir 'launch-dialog.txt'
Remove-Item -Force -ErrorAction SilentlyContinue $DialogFile
$env:COCKPIT_BROWSER = $FakeBrowser
$env:COCKPIT_LAUNCH_DIALOG_FILE = $DialogFile
$env:COCKPIT_NO_UPDATE_CHECK = '1'

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

# Inno log 中某段標記的出現次數；log 不存在回 -1。複製檔案每個 [Files] 項目一段 "-- File entry --"，
# 執行 [Run] 項目每筆一段 "-- Run entry --"（Inno 6.7.1 Setup.Install.pas、Setup.MainFunc.pas）。
function Count-LogMarker([string] $Log, [string] $Marker) {
    if (-not (Test-Path $Log -PathType Leaf)) { return -1 }
    return @(Select-String -Path $Log -SimpleMatch $Marker).Count
}

function Count-FileEntries([string] $LogName) {
    return (Count-LogMarker (Join-Path $LogDir "$LogName.log") '-- File entry --')
}

# 以啟動器交棒的同一組參數（cockpit::update::installer_args，design D8）啟動安裝檔，不等待；呼叫端用 Wait-Setup 收結束碼。
# ArgumentList 對含空白的引數整個加雙引號（"/LOG=C:\a b\setup.log"），與啟動器用的 Rust Command::arg 相同。
# 不用 Start-Process -Wait：它等整個程序樹，會連安裝檔重新啟動的後端一起等。
function Start-UpdateSetup([string] $Log) {
    $psi = [Diagnostics.ProcessStartInfo]::new($Setup)
    foreach ($arg in '/SILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/NOCANCEL', '/COCKPITUPDATE=1', "/LOG=$Log") {
        $psi.ArgumentList.Add($arg)
    }
    $psi.UseShellExecute = $false
    return [Diagnostics.Process]::Start($psi)
}

# 等安裝檔結束並回傳結束碼；逾時（代表卡住，例如訊息框沒被抑制）就結束整個程序樹並回傳說明文字，讓檢查報 FAIL。
function Wait-Setup([Diagnostics.Process] $Process, [int] $TimeoutSec = 120) {
    if (-not $Process.WaitForExit($TimeoutSec * 1000)) {
        $Process.Kill($true)
        return "still running after $TimeoutSec s (killed)"
    }
    return $Process.ExitCode
}

# 更新模式等候迴圈寫入 log 的那一行（ai-cockpit.iss 的 PrepareToInstall）：回傳 "<秒數>,<狀態>"，沒有該行回 'missing'。
function Get-UpdateWait([string] $Log) {
    if (-not (Test-Path $Log -PathType Leaf)) { return 'missing' }
    $m = Select-String -Path $Log -Pattern 'Update mode: waited (\d+) s for Cockpit to exit, state (\d+)' |
        Select-Object -First 1
    if (-not $m) { return 'missing' }
    return "$($m.Matches[0].Groups[1].Value),$($m.Matches[0].Groups[2].Value)"
}

# 以資料目錄為工作目錄背景啟動已安裝的 cockpit.exe（與第 3 步相同），等到 /api/state 回應為 Cockpit。
function Start-InstalledServer([string] $Name) {
    $proc = Start-Process -FilePath (Join-Path $AppDir 'cockpit.exe') -WorkingDirectory $DataDir -PassThru `
        -WindowStyle Hidden -RedirectStandardOutput (Join-Path $LogDir "$Name.out") `
        -RedirectStandardError (Join-Path $LogDir "$Name.err")
    Check "$Name serves /api/state" $true (Wait-CockpitState 30)
    return $proc
}

# 127.0.0.1:7770/api/state 是否回應 Cockpit：JSON 物件且有 version 與 runtimes（與啟動器 launch::classify_response 相同）。
function Test-CockpitState {
    try {
        $state = Invoke-RestMethod -Uri 'http://127.0.0.1:7770/api/state' -TimeoutSec 2
    } catch {
        return $false
    }
    if ($state -isnot [pscustomobject]) { return $false }
    $names = @($state.PSObject.Properties.Name)
    return ($names -contains 'version') -and ($names -contains 'runtimes')
}

function Wait-CockpitState([int] $TimeoutSec) {
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while ((Get-Date) -lt $deadline) {
        if (Test-CockpitState) { return $true }
        Start-Sleep -Milliseconds 500
    }
    return $false
}

# 程式目錄裡的 cockpit.exe／cockpit-launch.exe 程序（依完整路徑比對，不碰其他同名程式）。
function Get-InstalledCockpitProcesses {
    $prefix = $AppDir.TrimEnd('\') + '\'
    return @(Get-CockpitProcesses | Where-Object { $_.Path -and $_.Path.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase) })
}

function Stop-InstalledCockpit {
    foreach ($proc in Get-InstalledCockpitProcesses) {
        Write-Host "      stopping $($proc.Path) (pid $($proc.Id))"
        Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
        $proc.WaitForExit(10000) | Out-Null
    }
}

# 刪掉資料目錄中不在 $Keep 裡的項目（更新情境的後端寫的 cockpit.log 等），還原第 6 步「資料目錄是空的」的前提。
function Reset-DataDir([string[]] $Keep) {
    foreach ($item in @(Get-ChildItem -Force $DataDir)) {
        if ($Keep -notcontains $item.Name) {
            Write-Host "      removing $($item.FullName)"
            Remove-Item -Recurse -Force $item.FullName
        }
    }
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
        # 捷徑沒有另外指定圖示，沿用目標 cockpit-launch.exe 內嵌的應用程式圖示（spec「捷徑顯示應用程式圖示」；
        # change app-icon）。未指定時 IconLocation 為 ",0"（install-desktop.ps1 建的捷徑實測），也接受明確寫成目標本身。
        $target = Join-Path $AppDir 'cockpit-launch.exe'
        $iconFromTarget = ($lnk.IconLocation -eq ',0') -or ($lnk.IconLocation -ieq "$target,0")
        if (-not $iconFromTarget) { Write-Host "      IconLocation: $($lnk.IconLocation)" }
        Check "$Label shortcut icon comes from cockpit-launch.exe" $true $iconFromTarget
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

# 應用程式圖示與版本資訊（spec release-distribution「應用程式圖示與版本資訊」；change app-icon design D5）。
# 圖示數量：shell32 的 ExtractIconExW 以 nIconIndex = -1、兩個輸出皆 NULL 呼叫時回傳 RT_GROUP_ICON 數，錯誤回傳 UINT_MAX。
# 只看數量分不出「我們的圖示」與 Inno 的預設圖示，所以另取 32 px 圖示和 packaging/icon/app.ico 逐像素比對
# （Inno 的 SetupIconFile 把 .ico 各影像原樣寫進安裝檔，issrc is-6_7_1 Compiler.ExeUpdateFunc.pas UpdateIconsAndStyle）。
# ExtractAssociatedIcon 的尺寸取系統大圖示（SM_CXICON）：runner 為 96 DPI 即 32 px。在縮放比例會讓它落在 .ico 沒有的
# 尺寸（例如 36 px）的機器上用 -AllowOnThisMachine 跑，比對可能誤判不符。
Add-Type -AssemblyName System.Drawing
Add-Type -Namespace CockpitSmoke -Name Shell -MemberDefinition @'
[DllImport("shell32.dll", CharSet = CharSet.Unicode)]
public static extern uint ExtractIconExW(string lpszFile, int nIconIndex, IntPtr phiconLarge, IntPtr phiconSmall, uint nIcons);
'@
$AppIco = Join-Path $PSScriptRoot 'icon\app.ico'

function Get-IconCount([string] $Path) {
    $n = [CockpitSmoke.Shell]::ExtractIconExW($Path, -1, [IntPtr]::Zero, [IntPtr]::Zero, 0)
    if ($n -eq [uint32]::MaxValue) { return -1 }
    return [int] $n
}

function Test-AppIcon([string] $Path) {
    $a = [System.Drawing.Icon]::ExtractAssociatedIcon($Path).ToBitmap()
    $b = (New-Object System.Drawing.Icon $AppIco, $a.Width, $a.Height).ToBitmap()
    if ($a.Width -ne $b.Width -or $a.Height -ne $b.Height) { return $false }
    for ($y = 0; $y -lt $a.Height; $y++) {
        for ($x = 0; $x -lt $a.Width; $x++) {
            if ($a.GetPixel($x, $y).ToArgb() -ne $b.GetPixel($x, $y).ToArgb()) { return $false }
        }
    }
    return $true
}

function Check-AppIcon([string] $Label, [string] $Path) {
    Check "$Label has an icon resource" $true ((Get-IconCount $Path) -ge 1)
    # 取圖或讀 .ico 丟例外時記為 FAIL 並印出原因，不讓整支腳本在 Stop 模式下中斷、後面的步驟都沒跑。
    try {
        $same = Test-AppIcon $Path
    } catch {
        Write-Host "      icon comparison error: $($_.Exception.Message)"
        $same = $false
    }
    Check "$Label icon matches packaging/icon/app.ico" $true $same
}

function Check-VersionInfo([string] $Label, [string] $Path) {
    $v = (Get-Item $Path).VersionInfo
    Check "$Label ProductName" $AppName $v.ProductName
    Check "$Label FileDescription" $AppName $v.FileDescription
    Check "$Label ProductVersion" $Version $v.ProductVersion
    Check "$Label FileVersion" $Version $v.FileVersion
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
# 已發出的啟動器以程式目錄有 unins000.exe 判定「由安裝檔安裝」（spec release-distribution「自動更新客戶端契約」第 4 項）。
Check 'installs unins000.exe (auto-update client contract)' $true (Test-Path (Join-Path $AppDir 'unins000.exe') -PathType Leaf)
Check-AppIcon 'setup program' $Setup
foreach ($exe in 'cockpit.exe', 'cockpit-launch.exe') {
    $p = Join-Path $AppDir $exe
    if (Test-Path $p) {
        Check-AppIcon $exe $p
        Check-VersionInfo $exe $p
    }
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

# 5a、5b 用啟動器交棒的同一組參數（/SILENT…/COCKPITUPDATE=1），log 放在含空白的目錄：啟動器的 %TEMP% 可能含空白
# （使用者名稱），/LOG= 的傳法要在這裡實測過。
$updateLogDir = Join-Path $LogDir 'update log dir'
New-Item -ItemType Directory -Force $updateLogDir | Out-Null
$dataKeep = @(Get-ChildItem -Force $DataDir | ForEach-Object { $_.Name })

Step '5a. Update mode waits for Cockpit to exit, installs, and relaunches'
$waitLog = Join-Path $updateLogDir 'setup.log'
$server = Start-InstalledServer 'cockpit-update-wait'
$updateSetup = Start-UpdateSetup $waitLog
try {
    # 計時器：5 秒後停掉執行中的 cockpit.exe；安裝檔要在這段期間等候，而不是像非更新模式那樣立即以 7 結束。
    Start-Sleep -Seconds 5
    Check 'update-mode setup is still waiting after 5 s' $false $updateSetup.HasExited
    Stop-Process -Id $server.Id -Force -ErrorAction SilentlyContinue
    $server.WaitForExit()
    Check 'update-mode setup exit code' 0 (Wait-Setup $updateSetup)
    Write-Host "      update-mode log: $waitLog"
    Check 'update-mode log written to the path with spaces' $true (Test-Path $waitLog -PathType Leaf)
    $waited = Get-UpdateWait $waitLog
    Check 'update-mode setup waited, then saw Cockpit exit (state 0)' $true `
        ($waited -match '^(\d+),0$' -and [int]$Matches[1] -ge 1 -and [int]$Matches[1] -le 30)
    Write-Host "      waited,state: $waited"
    Check 'update-mode log records copied files' $true ((Count-LogMarker $waitLog '-- File entry --') -gt 0)
    Check 'update-mode log records the relaunch entry' 1 (Count-LogMarker $waitLog '-- Run entry --')
    # 重新啟動的啟動器拉起了後端：本腳本起的 cockpit.exe 已停掉，7770 上的 Cockpit 只能是它啟動的。
    Check 'relaunched launcher started the backend (/api/state is Cockpit within 30 s)' $true (Wait-CockpitState 30)
    $owner = @(Get-NetTCPConnection -LocalPort 7770 -State Listen -ErrorAction SilentlyContinue |
            Select-Object -ExpandProperty OwningProcess -Unique)
    $ownerPath = ($owner | ForEach-Object { (Get-Process -Id $_ -ErrorAction SilentlyContinue).Path }) -join ','
    Check 'port 7770 is owned by the installed cockpit.exe' (Join-Path $AppDir 'cockpit.exe') $ownerPath
    # 啟動器以工作目錄決定 cockpit.log 的位置（沒有 cockpit.toml 時寫在工作目錄）。
    Check 'relaunched launcher ran in the data directory (cockpit.log there)' $true `
        (Test-Path (Join-Path $DataDir 'cockpit.log') -PathType Leaf)
    # 啟動器拉起後端、開完（假）瀏覽器就該自行結束，不能卡住。
    $deadline = (Get-Date).AddSeconds(15)
    while (@(Get-InstalledCockpitProcesses | Where-Object Name -eq 'cockpit-launch').Count -gt 0 -and
        (Get-Date) -lt $deadline) {
        Start-Sleep -Milliseconds 200
    }
    Check 'relaunched launcher exits on its own within 15 s' 0 `
        @(Get-InstalledCockpitProcesses | Where-Object Name -eq 'cockpit-launch').Count
} finally {
    if (-not $updateSetup.HasExited) { $updateSetup.Kill($true) }
    Stop-InstalledCockpit
}
Reset-DataDir $dataKeep

Step '5b. Update mode gives up after 30 s while Cockpit keeps running'
$timeoutLog = Join-Path $updateLogDir 'setup-timeout.log'
$before = Get-FileHashes
$server = Start-InstalledServer 'cockpit-update-timeout'
$clock = [Diagnostics.Stopwatch]::StartNew()
$updateSetup = Start-UpdateSetup $timeoutLog
try {
    # 等候期間與結束後 3 秒輪詢程式目錄的 cockpit-launch.exe；權威證據是 log 沒有 [Run] 紀錄，輪詢是補充。
    $launchSeen = 0
    $deadline = (Get-Date).AddSeconds(120)
    while (-not $updateSetup.HasExited -and (Get-Date) -lt $deadline) {
        $launchSeen += @(Get-InstalledCockpitProcesses | Where-Object Name -eq 'cockpit-launch').Count
        Start-Sleep -Milliseconds 200
    }
    $code = Wait-Setup $updateSetup 1
    $elapsed = $clock.Elapsed.TotalSeconds
    $settle = (Get-Date).AddSeconds(3)
    while ((Get-Date) -lt $settle) {
        $launchSeen += @(Get-InstalledCockpitProcesses | Where-Object Name -eq 'cockpit-launch').Count
        Start-Sleep -Milliseconds 200
    }
    Write-Host "      update-mode log: $timeoutLog"
    Write-Host ("      update-mode setup took {0:N1} s" -f $elapsed)
    # /SILENT /SUPPRESSMSGBOXES 遇到 PrepareToInstall 中止時不卡在訊息框（卡住會被 Wait-Setup 判逾時）。
    Check 'update-mode setup while running exits with 7 (cannot proceed)' 7 $code
    Check 'update-mode setup waited at least 30 s' $true ($elapsed -ge 30)
    $waited = Get-UpdateWait $timeoutLog
    Write-Host "      waited,state: $waited"
    Check 'update-mode setup waited 30 s and Cockpit was still running (state 1)' '30,1' $waited
    Check 'update-mode setup while running copies no files' 0 (Count-LogMarker $timeoutLog '-- File entry --')
    Check 'update-mode setup while running runs no [Run] entry' 0 (Count-LogMarker $timeoutLog '-- Run entry --')
    Check 'update-mode setup while running starts no cockpit-launch.exe' 0 $launchSeen
    Check 'update-mode setup while running leaves files in place' $before (Get-FileHashes)
    Check 'update-mode setup while running leaves cockpit.exe running' $false $server.HasExited
} finally {
    if (-not $updateSetup.HasExited) { $updateSetup.Kill($true) }
    Stop-InstalledCockpit
}
Reset-DataDir $dataKeep

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

Step 'Cleanup: no Cockpit left running, no launcher messages'
# 停掉本腳本起的程序，不留給 workflow 後續步驟（也避免它們握著 step 的輸出管線）。
Stop-InstalledCockpit
Check 'no cockpit process left running' 0 @(Get-CockpitProcesses).Count
$dialog = if (Test-Path $DialogFile) { "$(Get-Content -Raw $DialogFile)".Trim() } else { '' }
Check 'launcher showed no message (COCKPIT_LAUNCH_DIALOG_FILE is empty)' '' $dialog

Write-Host ''
if ($script:Failures -gt 0) {
    Write-Host "$($script:Failures) check(s) failed."
    exit 1
}
Write-Host 'All smoke checks passed.'
