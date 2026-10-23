; Instalador de EdenBord (Inno Setup 6).
; Uso: iscc /DVersion=0.1.0 packaging\windows\edenbord.iss
; Espera target\release\edenbord.exe ya compilado; deja el instalador en target\paquetes\.

#ifndef Version
  #define Version "0.0.0"
#endif

[Setup]
AppId={{6F1B7C0E-3D2A-4E8B-9C5F-EDE0B0D0A001}
AppName=EdenBord
AppVersion={#Version}
AppPublisher=Julio Contreras
DefaultDirName={autopf}\EdenBord
DefaultGroupName=EdenBord
DisableProgramGroupPage=yes
LicenseFile=..\..\LICENSE
OutputDir=..\..\target\paquetes
OutputBaseFilename=EdenBord-{#Version}-instalador
SetupIconFile=..\..\assets\edenbord.ico
UninstallDisplayIcon={app}\edenbord.exe
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequiredOverridesAllowed=dialog
ChangesAssociations=yes

[Languages]
Name: "es"; MessagesFile: "compiler:Languages\Spanish.isl"

[Tasks]
Name: "escritorio"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "asociarjef"; Description: "Abrir los archivos .jef con EdenBord"; GroupDescription: "Asociar archivos:"

[Files]
Source: "..\..\target\release\edenbord.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"

[Icons]
Name: "{group}\EdenBord"; Filename: "{app}\edenbord.exe"
Name: "{autodesktop}\EdenBord"; Filename: "{app}\edenbord.exe"; Tasks: escritorio

[Registry]
; Proyectos .edenbord: siempre.
Root: HKA; Subkey: "Software\Classes\.edenbord"; ValueType: string; ValueName: ""; ValueData: "EdenBord.Proyecto"; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\EdenBord.Proyecto"; ValueType: string; ValueName: ""; ValueData: "Proyecto EdenBord"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\EdenBord.Proyecto\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\edenbord.exe,0"
Root: HKA; Subkey: "Software\Classes\EdenBord.Proyecto\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\edenbord.exe"" ""%1"""
; Bordados .jef: en "Abrir con" siempre; como programa por defecto si se elige la tarea.
Root: HKA; Subkey: "Software\Classes\.jef\OpenWithProgids"; ValueType: string; ValueName: "EdenBord.Jef"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.jef"; ValueType: string; ValueName: ""; ValueData: "EdenBord.Jef"; Flags: uninsdeletevalue; Tasks: asociarjef
Root: HKA; Subkey: "Software\Classes\EdenBord.Jef"; ValueType: string; ValueName: ""; ValueData: "Bordado Janome"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\EdenBord.Jef\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\edenbord.exe,0"
Root: HKA; Subkey: "Software\Classes\EdenBord.Jef\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\edenbord.exe"" ""%1"""

[Run]
Filename: "{app}\edenbord.exe"; Description: "{cm:LaunchProgram,EdenBord}"; Flags: nowait postinstall skipifsilent
