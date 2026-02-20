; Fundval installer (Inno Setup)

#define MyAppName "Fundval"
#define MyAppPublisher "Fundval"
#define MyAppURL "https://github.com/"

#ifndef MyAppVersion
#define MyAppVersion "0.0.0"
#endif

#ifndef SourceDir
#define SourceDir ".\\dist\\fundval"
#endif

[Setup]
AppId={{CAC418BE-2E30-4B30-9CEE-AE54776464E7}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={localappdata}\{#MyAppName}
DisableDirPage=no
DisableProgramGroupPage=yes
OutputDir=.
OutputBaseFilename=Fundval-Setup-{#MyAppVersion}-x64
Compression=lzma
SolidCompression=yes
PrivilegesRequired=lowest

[Languages]
Name: "chinesesimp"; MessagesFile: "compiler:Default.isl"

[Files]
Source: "{#SourceDir}\\*"; DestDir: "{app}"; Flags: recursesubdirs createallsubdirs ignoreversion

[Icons]
Name: "{autoprograms}\\{#MyAppName}"; Filename: "{app}\\start.bat"

[Run]
Filename: "{app}\\start.bat"; Description: "启动 Fundval"; Flags: nowait postinstall skipifsilent

