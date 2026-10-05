; Build after `cargo build --release` with Inno Setup 6.
#define MyAppName "Auris"
#define MyAppVersion "0.2.1"
#define MyAppPublisher "Victor Ibironke"
#define MyAppExeName "auris.exe"

[Setup]
AppId={{EBA9E9B0-5ED4-4C1F-BD2E-6F1D36D9E5F4}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher={#MyAppPublisher}
DefaultDirName={localappdata}\Programs\Auris
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
OutputDir=..\dist
OutputBaseFilename=AurisSetup-{#MyAppVersion}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
SetupIconFile=..\assets\auris.ico
WizardSmallImageFile=wizard-small-1x.bmp,wizard-small-2x.bmp
WizardImageFile=wizard-large-1x.bmp,wizard-large-2x.bmp
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayIcon={app}\{#MyAppExeName}
UninstallDisplayName={#MyAppName}
; Close a running Auris before upgrading or uninstalling.
CloseApplications=force
RestartApplications=no

[Tasks]
Name: "startup"; Description: "Start Auris when I sign in (recommended)"; GroupDescription: "Startup:"
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Shortcuts:"; Flags: unchecked

[Files]
Source: "..\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; DestName: "README.txt"; Flags: ignoreversion

[Icons]
Name: "{userprograms}\Auris"; Filename: "{app}\{#MyAppExeName}"
Name: "{userdesktop}\Auris"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon
Name: "{userstartup}\Auris"; Filename: "{app}\{#MyAppExeName}"; Parameters: "--background"; Tasks: startup

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "Launch Auris"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "{cmd}"; Parameters: "/C taskkill /IM {#MyAppExeName} /F"; Flags: runhidden; RunOnceId: "StopAuris"

[UninstallDelete]
; Settings, history, and logs live in %LOCALAPPDATA%\Auris and are kept on purpose.
Type: filesandordirs; Name: "{app}"
