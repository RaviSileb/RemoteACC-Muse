# GUI aplikace pro Windows

Desktopová GUI je stejná Flutter aplikace jako pro Android
([clients/racc_app](../clients/racc_app/README.md)) — jeden kód,
platforma Windows. Rozhodnutí Flutter vs Tauri (otevřená volba č. 3
v roadmapě) je tímto uzavřeno ve prospěch Flutteru.

## Sestavení

```powershell
# z kořene repa
cargo build --release -p racc-core
cd clients\racc_app
flutter build windows --release
Copy-Item ..\..\target\release\racc_core.dll build\windows\x64\runner\Release\
```

`racc_core.dll` musí ležet vedle `racc_app.exe`, jinak aplikace
nenačte nativní jádro (hláška na úvodní obrazovce).

## Instalace

```powershell
# z kořene repa (vygeneruje i admin heslo a vypíše ho)
powershell -ExecutionPolicy Bypass -File scripts\install-gui.ps1

# s vlastním heslem + vlastním cílem:
powershell -ExecutionPolicy Bypass -File scripts\install-gui.ps1 `
  -AdminPassword 'tajne-heslo' -InstallDir 'C:\RemoteACC'
```

Instalátor (`scripts/install-gui.ps1`):

- zkopíruje sestavení do `%LOCALAPPDATA%\Programs\racc_app` (výchozí),
- vytvoří admin účet do `%APPDATA%\RemoteACC\admin.json`
  (`{username, salt, hash}`, hash = SHA-256 hex z `"sůl:heslo"`),
- přidá zástupce RemoteACC do nabídky Start,
- vypíše heslo (zobrazí se jen jednou — uložte si ho).

Odinstalace: smazat instalační adresář, `%APPDATA%\RemoteACC`
a zástupce z nabídky Start.

## Přihlášení

Po startu vyžaduje aplikace login `admin` + heslo z instalace.
Ověření je lokální (sůl + SHA-256, viz `lib/auth.dart`) — jde o bránu
aplikace, ne o serverovou autentizaci. Stejné heslo funguje i v Android
aplikaci, pokud se APK sestaví se stejnými `--dart-define` hodnotami
(viz [ANDROID.md](ANDROID.md)).

Změna hesla: spusťte instalátor znovu s `-AdminPassword` (přepíše
`admin.json`, soubory aplikace jen překopíruje).

## Testy

- `flutter analyze`, `flutter test` v `clients/racc_app` (auth logika
  proti nezávislému SHA-256 vektoru + FFI test přes hostitelskou DLL).
- Smoke test instalace: start `racc_app.exe`, kontrola okna, login.
