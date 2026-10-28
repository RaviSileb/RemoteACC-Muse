# Vývoj Android aplikace (cíl: Galaxy S23 Ultra)

Nativní klient je Flutter aplikace [clients/racc_app](../clients/racc_app/README.md)
s UI nad `racc-core` přes Dart FFI. Stav: funkční kostra (verze jádra,
generování identity, validace/parsování `racc://` URI); plný klient
(TTY, soubory, video) je fáze 4 dle [ROADMAP](ROADMAP.md).

## Cílové zařízení

- Samsung Galaxy S23 Ultra (SM-S918B), Snapdragon 8 Gen 2 → **arm64-v8a**,
  Android 13/14 (One UI), 120Hz displej (Impeller).
- APK se staví **jen pro `arm64-v8a`** (`abiFilters` v
  `android/app/build.gradle.kts`) — menší balíček, na S23 Ultra stačí.
- Na telefonu: Vývojářské možnosti → Ladění USB; připojit kabelem a povolit
  počítač (`adb devices` musí ukázat `device`, ne `unauthorized`).

## Předpoklady na PC

- Flutter 3.41 (`C:\Users\ivarb\flutter`), Android SDK + NDK 28
  (`%LOCALAPPDATA%\Android\Sdk`), Rust target `aarch64-linux-android`,
  `cargo-ndk`.
- Ověření: `flutter doctor` (vše zelené), `rustup target list --installed`,
  `cargo ndk --version`.
- Známý problém: rozbitý `pwsh.exe` alias z Microsoft Storu
  (`App execution aliases` bez nainstalované aplikace) rozbije `flutter.bat`
  chybou `Unable to determine engine version`. Řešení: buď nainstalovat
  PowerShell 7, nebo spouštět Flutter s PATH bez `WindowsApps`:
  ```powershell
  $env:PATH = ($env:PATH -split ';' | Where-Object { $_ -notlike '*WindowsApps*' }) -join ';'
  ```

## Sestavení

```powershell
# 1) Nativní knihovna pro arm64 (výstup: android/app/src/main/jniLibs/arm64-v8a/)
$env:ANDROID_NDK_HOME = "$env:LOCALAPPDATA\Android\Sdk\ndk\28.2.13676358"
cargo ndk -t arm64-v8a `
  -o clients\racc_app\android\app\src\main\jniLibs `
  build --release -p racc-core

# 2) APK (debug, podepsáno debug klíčem)
flutter build apk --debug clients\racc_app   # z kořene repa, nebo z clients\racc_app bez cesty
# výstup: clients\racc_app\build\app\outputs\flutter-apk\app-debug.apk
```

Kontrola obsahu APK (musí obsahovat `lib/arm64-v8a/libracc_core.so`):

```powershell
python -c "import zipfile; print([n for n in zipfile.ZipFile('app-debug.apk').namelist() if n.endswith('.so')])"
```

## Instalace a spuštění na S23 Ultra

```powershell
adb devices            # telefon připojen a autorizován
adb install -r app-debug.apk
# nebo přímý vývojový běh s hot-reloadem:
flutter run
```

Tipy: `adb logcat` pro logy, `flutter devtools` pro inspekci,
`adb install -r` pro reinstalaci při zachování dat.

## Přihlášení (admin)

Aplikace po startu vyžaduje login `admin` + heslo. Ověření je lokální
(sůl + SHA-256, `lib/auth.dart`) — brána aplikace, ne serverová
autentizace. Kredence se do APK dostanou při buildu:

```powershell
flutter build apk --debug `
  --dart-define=ADMIN_SALT=<64hex> --dart-define=ADMIN_HASH=<64hex>
```

kde `ADMIN_HASH = SHA-256 hex("ADMIN_SALT:heslo")`. Stejné heslo jako
Windows GUI: vygenerujte jednou a použijte pro instalátor (`-AdminPassword`)
i pro APK. Sůl+hash nikdy necommitujte do repa.

E2E test loginu na zařízení: `adb shell input tap` do pole hesla,
`adb shell input text <heslo>` (heslo bez diakritiky a mezer),
`adb shell input keyevent 66`, kontrola screenshotem.

## Testy

- `flutter analyze` v `clients/racc_app` — musí být bez nálezů.
- `flutter test` — hostitelský FFI test (`test/ffi_test.dart`) volá skutečné
  funkce `racc_core` (verze, identita, fingerprint, URI). Vyžaduje
  `cargo build -p racc-core` (debug) v kořeni repa.
- E2E na zařízení je manuální (aplikace → Nová identita / Ověřit URI).

## Co ještě chybí (fáze 4)

Plný klient nad C-ABI (TTY, přenos souborů, registr přes bootstrap),
MediaProjection (sdílení obrazovky), AccessibilityService (vstup),
ICE/STUN-TURN hole-punching, podepsaný release build a repozitářová CI
pro Flutter (`analyze` + `test` + `build apk`).
