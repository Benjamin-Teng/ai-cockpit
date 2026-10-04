; AI Agent Cockpit 安裝精靈（Inno Setup 6）。規格：openspec release-distribution「Windows 安裝檔」「解除安裝」；
; 取捨見 change release-packaging 的 design D1、D2、D7。
; 更新模式（/COCKPITUPDATE=1，啟動器自動更新交棒時帶）：change auto-update 的 design D8、D9。
;
; 由 .github/workflows/release.yml 編譯：
;   iscc -dAppVersion=0.1.0 -dStageDir=<staging 目錄> -dOutputDir=<輸出目錄> packaging\ai-cockpit.iss
; staging 目錄要有 cockpit.exe、cockpit-launch.exe、cockpit.example.toml、LICENSE.txt。

#ifndef AppVersion
  #error AppVersion is required: pass -dAppVersion=X.Y.Z
#endif
#ifndef StageDir
  #error StageDir is required: pass -dStageDir=<directory with the files to install>
#endif
#ifndef OutputDir
  #error OutputDir is required: pass -dOutputDir=<directory for the setup program>
#endif

#define AppName "AI Agent Cockpit"
; 資料目錄：捷徑與完成頁啟動的工作目錄（cockpit.toml、cockpit.log、狀態檔放這裡），與 install-desktop.ps1
; 零設定時相同（design D1）。解除安裝不刪。
#define DataDir "{localappdata}\ai-cockpit"

[Setup]
AppId=BenjaminTeng.AIAgentCockpit
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=Benjamin-Teng
AppPublisherURL=https://github.com/Benjamin-Teng/ai-cockpit
AppSupportURL=https://github.com/Benjamin-Teng/ai-cockpit/issues
AppUpdatesURL=https://github.com/Benjamin-Teng/ai-cockpit/releases
VersionInfoVersion={#AppVersion}
; 只裝給目前使用者：{autopf} 在 non-admin 模式為 %LOCALAPPDATA%\Programs。不設
; PrivilegesRequiredOverridesAllowed，使用者不能改成所有使用者安裝。
PrivilegesRequired=lowest
DefaultDirName={autopf}\{#AppName}
DisableProgramGroupPage=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#OutputDir}
OutputBaseFilename=ai-cockpit-{#AppVersion}-x64-setup
LicenseFile={#StageDir}\LICENSE.txt
UninstallDisplayName={#AppName}
UninstallDisplayIcon={app}\cockpit-launch.exe
; 安裝檔本身與精靈視窗的圖示（change app-icon design D4）；路徑相對於本檔所在的 packaging/。
SetupIconFile=icon\app.ico
; 執行中的偵測由 [Code] 負責，只提示、不關閉程式（design D2）。
CloseApplications=no
WizardStyle=modern
; 預設 yes 會每次先跳「選擇語言」；auto＝系統語言對得上就直接用（design D7）。
ShowLanguageDialog=auto
Compression=lzma2
SolidCompression=yes

; 依系統語言自動選。runner 的 Inno Setup 6.7.1 沒有內建繁中，ChineseTraditional.isl 取自 Inno 原始碼
; jrsoftware/issrc tag is-6_7_1 的 Files/Languages/Unofficial/（blob b8a50d595fe5dda3308fcbf27c4ce8ffddb2c9cc，design D7）。
[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "zh"; MessagesFile: "ChineseTraditional.isl"

[CustomMessages]
en.CockpitRunning=AI Agent Cockpit is running. Close the Cockpit window, wait about 10 seconds for it to shut down, then try again.
en.CockpitRunningUnknown=Setup could not check whether AI Agent Cockpit is running. Close Cockpit if it is open, then try again.
zh.CockpitRunning=AI Agent Cockpit 正在執行。請關閉 Cockpit 視窗，等候約 10 秒讓它結束後再試一次。
zh.CockpitRunningUnknown=安裝程式無法確認 AI Agent Cockpit 是否正在執行。如果 Cockpit 開著，請先關閉後再試一次。

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"

[Dirs]
Name: "{#DataDir}"; Flags: uninsneveruninstall

[Files]
Source: "{#StageDir}\cockpit.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#StageDir}\cockpit-launch.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#StageDir}\cockpit.example.toml"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#StageDir}\LICENSE.txt"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\cockpit-launch.exe"; WorkingDir: "{#DataDir}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\cockpit-launch.exe"; WorkingDir: "{#DataDir}"; Tasks: desktopicon

[Run]
; WorkingDir 不可省略：預設是程式目錄，會讀不到資料目錄的 cockpit.toml，cockpit.log 也會留在程式目錄。
Filename: "{app}\cockpit-launch.exe"; WorkingDir: "{#DataDir}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent
; 更新模式：裝完重新啟動啟動器（不帶引數）。啟動器交棒時一定帶靜默參數；一般靜默安裝沒有 /COCKPITUPDATE=1，
; Check 不成立，仍不啟動任何程式。
Filename: "{app}\cockpit-launch.exe"; WorkingDir: "{#DataDir}"; Flags: nowait skipifnotsilent; Check: IsUpdateMode

[Code]
// 程式目錄（Dir）中的 cockpit.exe 或 cockpit-launch.exe 是否正在執行。
// 回傳 0＝沒有、1＝有、2＝無法確認（WMI 失敗，或同名程序的路徑讀不到）。無法確認時呼叫端一律中止。
function CockpitRunState(Dir: String): Integer;
var
  Locator, Service, Procs, Proc, ExePath: Variant;
  Prefix, PathStr: String;
  I, Count: Integer;
begin
  Result := 0;
  Prefix := Lowercase(AddBackslash(Dir));
  try
    Locator := CreateOleObject('WbemScripting.SWbemLocator');
    Service := Locator.ConnectServer('.', 'root\CIMV2');
    Procs := Service.ExecQuery('SELECT ExecutablePath FROM Win32_Process ' +
      'WHERE Name = ''cockpit.exe'' OR Name = ''cockpit-launch.exe''');
    // Variant 不能直接當字串函式的引數或 for 的邊界（編譯期 Type mismatch），先指派給具型別變數轉換。
    Count := Procs.Count;
    for I := 0 to Count - 1 do
    begin
      Proc := Procs.ItemIndex(I);
      ExePath := Proc.ExecutablePath;
      if VarIsNull(ExePath) or VarIsEmpty(ExePath) then
        Result := 2
      else
      begin
        PathStr := ExePath;
        if Pos(Prefix, Lowercase(PathStr)) = 1 then
        begin
          Result := 1;
          Exit;
        end;
      end;
    end;
  except
    Result := 2;
  end;
end;

function RunningMessage(State: Integer): String;
begin
  if State = 1 then
    Result := CustomMessage('CockpitRunning')
  else
    Result := CustomMessage('CockpitRunningUnknown');
end;

// 更新模式：命令列帶 /COCKPITUPDATE=1（啟動器交棒時帶，design D8、D9）。
function IsUpdateMode(): Boolean;
begin
  Result := ExpandConstant('{param:COCKPITUPDATE|0}') = '1';
end;

// 安裝：在複製任何檔案前檢查；回傳非空字串即中止（靜默模式以結束碼 7 結束）。
// 更新模式先等啟動器與後端自行結束：每秒重查一次，最多等 30 秒，仍在執行或無法確認才中止。
function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  State, Waited: Integer;
begin
  Result := '';
  State := CockpitRunState(ExpandConstant('{app}'));
  if IsUpdateMode() then
  begin
    Waited := 0;
    while (State <> 0) and (Waited < 30) do
    begin
      Sleep(1000);
      Waited := Waited + 1;
      State := CockpitRunState(ExpandConstant('{app}'));
    end;
    Log('Update mode: waited ' + IntToStr(Waited) + ' s for Cockpit to exit, state ' + IntToStr(State));
  end;
  if State <> 0 then
    Result := RunningMessage(State);
end;

// 解除安裝：在移除任何檔案前檢查；SuppressibleMsgBox 在 /SUPPRESSMSGBOXES 時不跳出（一般 MsgBox 會卡住靜默模式）。
function InitializeUninstall(): Boolean;
var
  State: Integer;
begin
  Result := True;
  State := CockpitRunState(ExpandConstant('{app}'));
  if State <> 0 then
  begin
    SuppressibleMsgBox(RunningMessage(State), mbError, MB_OK, IDOK);
    Result := False;
  end;
end;
