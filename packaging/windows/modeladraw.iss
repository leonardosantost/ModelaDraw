; Inno Setup script for ModelaDraw. Built by CI on windows-latest.
; Variables: /DVersion=x.y.z /DSrcDir=<dir with modeladraw.exe and tracedraw-cli.exe>

#ifndef Version
  #define Version "0.0.0"
#endif
#ifndef SrcDir
  #define SrcDir "..\..\target\release"
#endif

[Setup]
AppId={{07B010F3-415B-4D51-B424-60BB11AB9837}
AppName=ModelaDraw
AppVersion={#Version}
AppPublisher=ModelaDraw
AppCopyright=Copyright (c) 2026 Ighor Toth and the TraceDraw contributors; ModelaDraw contributors
AppPublisherURL=https://github.com/leonardosantost/ModelaDraw
DefaultDirName={autopf}\ModelaDraw
DefaultGroupName=ModelaDraw
UninstallDisplayIcon={app}\modeladraw.exe
OutputDir=..\..\dist
OutputBaseFilename=modeladraw-{#Version}-windows-x86_64-setup
SetupIconFile=..\..\assets\icon\tracedraw.ico
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequiredOverridesAllowed=dialog
ChangesAssociations=yes
ChangesEnvironment=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
Name: "addtopath"; Description: "Add tracedraw-cli to PATH"; GroupDescription: "Command line:"

[Files]
Source: "{#SrcDir}\modeladraw.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SrcDir}\tracedraw-cli.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE-MIT"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE-APACHE"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\ModelaDraw"; Filename: "{app}\modeladraw.exe"
Name: "{group}\Uninstall ModelaDraw"; Filename: "{uninstallexe}"
Name: "{autodesktop}\ModelaDraw"; Filename: "{app}\modeladraw.exe"; Tasks: desktopicon

[Registry]
; .tdraw file association
Root: HKA; Subkey: "Software\Classes\.tdraw"; ValueType: string; ValueName: ""; ValueData: "ModelaDraw.Document"; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\ModelaDraw.Document"; ValueType: string; ValueName: ""; ValueData: "ModelaDraw Document"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\ModelaDraw.Document\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\modeladraw.exe,0"
Root: HKA; Subkey: "Software\Classes\ModelaDraw.Document\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\modeladraw.exe"" ""%1"""
; "Open with ModelaDraw" for .cdr, without stealing the default
Root: HKA; Subkey: "Software\Classes\.cdr\OpenWithProgids"; ValueType: string; ValueName: "ModelaDraw.Document"; ValueData: ""; Flags: uninsdeletevalue
; PATH
Root: HKA; Subkey: "{code:PathRegKey}"; ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}"; Tasks: addtopath; Check: NeedsAddPath('{app}')

[Run]
Filename: "{app}\modeladraw.exe"; Description: "{cm:LaunchProgram,ModelaDraw}"; Flags: nowait postinstall skipifsilent shellexec runasoriginaluser

[Code]
function PathRegKey(Param: string): string;
begin
  if IsAdminInstallMode then
    Result := 'SYSTEM\CurrentControlSet\Control\Session Manager\Environment'
  else
    Result := 'Environment';
end;

function NeedsAddPath(Param: string): boolean;
var
  OrigPath: string;
  Root: Integer;
begin
  if IsAdminInstallMode then Root := HKLM else Root := HKCU;
  if not RegQueryStringValue(Root, PathRegKey(''), 'Path', OrigPath) then
  begin
    Result := True;
    exit;
  end;
  Result := Pos(';' + Uppercase(ExpandConstant(Param)) + ';', ';' + Uppercase(OrigPath) + ';') = 0;
end;
