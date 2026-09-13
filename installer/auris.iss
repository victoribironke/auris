; Build after `cargo build --release` with Inno Setup.
#define MyAppName "Auris"
#define MyAppVersion "0.1.0"
#define MyAppExeName "auris.exe"

[Setup]
AppId={{EBA9E9B0-5ED4-4C1F-BD2E-6F1D36D9E5F4}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
DefaultDirName={localappdata}\Auris
DefaultGroupName={#MyAppName}
OutputDir=..\dist
OutputBaseFilename=AurisSetup
Compression=lzma
SolidCompression=yes
PrivilegesRequired=lowest

[Files]
Source: "..\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{userprograms}\Auris"; Filename: "{app}\{#MyAppExeName}"

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "Launch Auris"; Flags: nowait postinstall skipifsilent
