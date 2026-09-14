[Setup]
AppId={{019EB36D-2D44-7A10-A0D3-1DA29AA7865C}}
AppName=DataTree
AppVersion={#AppVersion}
AppPublisher=DataTree
AppPublisherURL=https://github.com/Xangelix/edirstat
AppSupportURL=https://github.com/Xangelix/edirstat/issues
AppUpdatesURL=https://github.com/Xangelix/edirstat/releases
DefaultDirName={autopf}\DataTree
DefaultGroupName=DataTree
DisableProgramGroupPage=yes
LicenseFile=LICENSE
; Output directory and name
OutputDir=staging
OutputBaseFilename=datatree-setup-x86_64
SetupIconFile=assets\img\icon.ico
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=admin

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "target\release\datatree.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "target\release\edirstat.exe"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "target\release\datatree-mcp.exe"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist

[Icons]
Name: "{group}\DataTree"; Filename: "{app}\datatree.exe"; IconFilename: "{app}\datatree.exe"
Name: "{autodesktop}\DataTree"; Filename: "{app}\datatree.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\datatree.exe"; Description: "{cm:LaunchProgram,DataTree}"; Flags: nowait postinstall skipifsilent
