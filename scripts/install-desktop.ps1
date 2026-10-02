<#
.SYNOPSIS
  建置 cockpit 的兩個執行檔、安裝到使用者目錄，並在桌面建立「AI Agent Cockpit」捷徑。

.DESCRIPTION
  desktop-launch-notify task 2.4（spec desktop-launch「桌面捷徑安裝腳本」）；修正波 2.6 M2、M4、M5。

  步驟：
    1. 執行中檢查（提早拒絕，省下白跑的建置）：安裝目錄的 cockpit.exe 或 cockpit-launch.exe
       正在執行時，提示先關閉 Cockpit 並以非 0 結束，不建置、不複製、不改捷徑。
    2. 在 repo 根目錄執行 cargo build --release -p cockpit --bins。
    3. 複製：先以「獨占」方式同時開啟安裝目錄的兩個目標檔（不存在就建立），兩個都開成功才寫入。
       建置可能要好幾分鐘，期間使用者可能點了捷徑，所以這一步自己再判定一次是否在執行；任一個開不了
       （執行中的映像檔無法以寫入、不共用的方式開啟）就兩個都不寫、提示先關閉 Cockpit 並以非 0 結束，
       不會留下一新一舊的兩支執行檔。握著兩個獨占 handle 期間別人也無法啟動它們，檢查與寫入之間沒有空檔。
    4. 在捷徑目錄建立 "AI Agent Cockpit.lnk"，目標為安裝目錄的 cockpit-launch.exe：
       - 設定檔存在：引數為 --config "<絕對路徑>"，工作目錄為設定檔所在目錄。
       - 設定檔不存在：警告將以零設定模式執行；不帶引數，工作目錄為
         %LOCALAPPDATA%\ai-cockpit\（必要時建立）。
  重複執行即為更新。

  執行中的判定不只看程序清單：32 位元的 Windows PowerShell 5.1 讀不到 64 位元程序的 Path
  （回空字串），以系統管理員或其他使用者身分執行的程序也可能讀不到，所以以「目標檔能否獨占開啟」為準，
  程序清單只用來在訊息中列出 PID。

  建置產物位置：環境變數 CARGO_TARGET_DIR 有值時用它（相對路徑以 repo 根目錄為基準，與 cargo 在
  repo 根目錄執行時的解讀相同），否則為 <repo>\target。.cargo/config.toml 的 build.target-dir 不支援。

  cargo 經 cmd.exe 執行並在 cmd 內把 stderr 併入 stdout：Windows PowerShell 5.1 在呼叫端導向錯誤串流
  （例如 & .\install-desktop.ps1 2>&1 | ...）時，會把原生程式的 stderr 行包成錯誤紀錄，配合本腳本的
  $ErrorActionPreference = 'Stop' 會在 cargo 印出第一行進度時中止。成敗只看結束碼。

.PARAMETER Config
  設定檔路徑。預設為 repo 根目錄（本腳本所在目錄的上一層）的 cockpit.toml。

.PARAMETER InstallDir
  安裝目錄。預設為 %LOCALAPPDATA%\ai-cockpit\bin。

.PARAMETER ShortcutDir
  捷徑所在目錄。預設為使用者桌面（[Environment]::GetFolderPath('Desktop')，
  涵蓋桌面被重導向的情況）。

.EXAMPLE
  pwsh scripts/install-desktop.ps1

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts\install-desktop.ps1 -Config D:\work\cockpit.toml

.EXAMPLE
  pwsh scripts/install-desktop.ps1 -InstallDir $env:TEMP\t\bin -ShortcutDir $env:TEMP\t\desk   # 測試用
#>
[CmdletBinding()]
param(
    [string]$Config,
    [string]$InstallDir,
    [string]$ShortcutDir
)

$ErrorActionPreference = 'Stop'

# 印錯誤到 stderr 並以 1 結束（不用 Write-Error：Stop 模式下會附一長串錯誤紀錄）。
function Fail([string]$Message) {
    [Console]::Error.WriteLine($Message)
    exit 1
}

$RepoRoot = Split-Path -Parent $PSScriptRoot
if (-not $Config) { $Config = Join-Path $RepoRoot 'cockpit.toml' }
if (-not $InstallDir) { $InstallDir = Join-Path $env:LOCALAPPDATA 'ai-cockpit\bin' }
if (-not $ShortcutDir) { $ShortcutDir = [Environment]::GetFolderPath('Desktop') }

# 轉成絕對路徑（路徑不必已存在；以目前目錄為基準）。
function Resolve-FullPath([string]$Path) {
    $base = (Get-Location).ProviderPath
    return [IO.Path]::GetFullPath([IO.Path]::Combine($base, $Path))
}
$InstallDir = Resolve-FullPath $InstallDir
$ShortcutDir = Resolve-FullPath $ShortcutDir
$Config = Resolve-FullPath $Config

$ExeNames = @('cockpit.exe', 'cockpit-launch.exe')

# --- 執行中判定 ----------------------------------------------------------------------

# 程序清單中執行檔路徑等於安裝目錄目標檔的程序（只用來在訊息中列出 PID；讀不到 Path 的程序略過，
# 由下面的獨占開啟判定）。
function Get-RunningFromInstallDir {
    $targets = $ExeNames | ForEach-Object { Join-Path $InstallDir $_ }
    $found = @()
    foreach ($proc in @(Get-Process -Name 'cockpit', 'cockpit-launch' -ErrorAction SilentlyContinue)) {
        $procPath = $null
        try { $procPath = $proc.Path } catch { }
        if (-not $procPath) { continue }
        foreach ($target in $targets) {
            if ([string]::Equals($procPath, $target, [StringComparison]::OrdinalIgnoreCase)) {
                $found += "$($proc.ProcessName).exe (PID $($proc.Id))"
            }
        }
    }
    return $found
}

# 以「讀寫、不與任何人共用」開啟安裝目錄的兩個目標檔；不存在的檔案就建立（$create 為真時）。
# 回傳 @{ Streams = 已開啟的串流; Created = 這次新建的路徑; Busy = 開不了的說明 }。
# 執行中的映像檔無法以這種方式開啟（共用違規），所以「開不了」就視為正在執行或被占用。
function Open-TargetsExclusive([bool]$create) {
    $streams = @()
    $created = @()
    $busy = @()
    foreach ($name in $ExeNames) {
        $path = Join-Path $InstallDir $name
        $exists = Test-Path -LiteralPath $path -PathType Leaf
        if (-not $exists -and -not $create) { continue }
        $mode = if ($exists) { [IO.FileMode]::Open } else { [IO.FileMode]::CreateNew }
        try {
            $streams += [IO.File]::Open($path, $mode, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None)
            if (-not $exists) { $created += $path }
        } catch {
            $inner = $_.Exception
            while ($inner.InnerException) { $inner = $inner.InnerException }
            $busy += "$name（$($inner.Message.Trim())）"
        }
    }
    return @{ Streams = $streams; Created = $created; Busy = $busy }
}

# 關掉串流；$discardCreated 為真時刪掉這次新建（仍是空檔）的目標檔，安裝目錄回到原狀。
function Close-Targets($opened, [bool]$discardCreated) {
    foreach ($stream in $opened.Streams) { $stream.Dispose() }
    if ($discardCreated) {
        foreach ($path in $opened.Created) { Remove-Item -LiteralPath $path -Force -ErrorAction SilentlyContinue }
    }
}

function Fail-Running([string[]]$Busy) {
    $running = @(Get-RunningFromInstallDir)
    $detail = @()
    if ($running.Count -gt 0) { $detail += ($running -join ', ') }
    if ($Busy.Count -gt 0) { $detail += ('無法獨占開啟：' + ($Busy -join '；')) }
    Fail ("安裝目錄 $InstallDir 的 Cockpit 正在執行或被占用：" + ($detail -join '；') +
        "。請先關閉 Cockpit（關掉視窗約 10 秒後後端自動結束）再重新執行；安裝目錄未做任何變更。")
}

# --- 1. 執行中檢查（提早拒絕；複製前會再以獨占開啟判定一次） -----------------------------
if (Test-Path -LiteralPath $InstallDir -PathType Container) {
    $probe = Open-TargetsExclusive $false
    Close-Targets $probe $false
    $runningEarly = @(Get-RunningFromInstallDir)
    if ($probe.Busy.Count -gt 0 -or $runningEarly.Count -gt 0) { Fail-Running $probe.Busy }
}

# --- 2. 建置 -----------------------------------------------------------------------
Write-Host "建置：cargo build --release -p cockpit --bins（$RepoRoot）"
Push-Location $RepoRoot
try {
    & cmd.exe /d /c 'cargo build --release -p cockpit --bins 2>&1'
    if ($LASTEXITCODE -ne 0) { Fail "cargo build 失敗（結束碼 $LASTEXITCODE）。" }
} finally {
    Pop-Location
}

$TargetDir = if ($env:CARGO_TARGET_DIR) {
    [IO.Path]::GetFullPath([IO.Path]::Combine($RepoRoot, $env:CARGO_TARGET_DIR))
} else {
    Join-Path $RepoRoot 'target'
}
$sources = @{}
foreach ($name in $ExeNames) {
    $src = Join-Path $TargetDir "release\$name"
    if (-not (Test-Path -LiteralPath $src -PathType Leaf)) { Fail "找不到建置產物：$src" }
    $sources[$name] = $src
}

# --- 3. 複製（兩個目標檔都獨占開啟成功才寫入） ---------------------------------------
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
$opened = Open-TargetsExclusive $true
if ($opened.Busy.Count -gt 0 -or @(Get-RunningFromInstallDir).Count -gt 0) {
    Close-Targets $opened $true
    Fail-Running $opened.Busy
}
try {
    for ($i = 0; $i -lt $ExeNames.Count; $i++) {
        $target = $opened.Streams[$i]
        $source = [IO.File]::OpenRead($sources[$ExeNames[$i]])
        try {
            $target.SetLength(0)
            $source.CopyTo($target)
        } finally {
            $source.Dispose()
        }
        Write-Host "已複製：$($ExeNames[$i]) -> $InstallDir"
    }
} finally {
    Close-Targets $opened $false
}

# --- 4. 捷徑 -----------------------------------------------------------------------
if (Test-Path -LiteralPath $Config -PathType Leaf) {
    $arguments = '--config "' + $Config + '"'
    $workDir = Split-Path -Parent $Config
} else {
    Write-Warning "找不到設定檔 $Config，捷徑將以零設定模式執行（不帶 --config）。"
    $arguments = ''
    $workDir = Join-Path $env:LOCALAPPDATA 'ai-cockpit'
    New-Item -ItemType Directory -Force -Path $workDir | Out-Null
}

New-Item -ItemType Directory -Force -Path $ShortcutDir | Out-Null
$lnkPath = Join-Path $ShortcutDir 'AI Agent Cockpit.lnk'
$shell = New-Object -ComObject WScript.Shell
$lnk = $shell.CreateShortcut($lnkPath)
$lnk.TargetPath = Join-Path $InstallDir 'cockpit-launch.exe'
$lnk.Arguments = $arguments
$lnk.WorkingDirectory = $workDir
$lnk.Description = 'AI Agent Cockpit'
$lnk.Save()

Write-Host "已建立捷徑：$lnkPath"
Write-Host "  目標：$($lnk.TargetPath)"
Write-Host "  引數：$arguments"
Write-Host "  工作目錄：$workDir"
Write-Host '完成。'
