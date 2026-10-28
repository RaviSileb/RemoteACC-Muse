# Flutter stub (fáze 4)

Kostra mobilního/desktopového GUI klienta. Plná aplikace vznikne ve fázi 4;
zde je hotový FFI kontrakt proti `racc-core` (cdylib).

## Sestavení knihovny

```sh
# Windows → racc_core.dll, Linux → libracc_core.so, macOS → .dylib
cargo build --release -p racc-core
# Android (NDK): cargo ndk -t arm64-v8a build --release -p racc-core
# iOS: cargo-lipo / xcframework dle docs/ROADMAP.md fáze 4
```

## Použití (Dart)

```dart
import 'package:racc_flutter_stub/racc_ffi.dart';

final racc = Racc.load('cesta/k/racc_core.dll');
final id = racc.newIdentity();          // {secret_hex, fingerprint}
racc.isValidUri('racc://vps:8443/pc#fp'); // true/false
```

`lib/racc_ffi.dart` pokrývá: `racc_version`, `racc_identity_generate`,
`racc_fingerprint_of`, `racc_uri_parse`, `racc_uri_is_valid`,
`racc_free_string`. Všechny vrácené řetězce se uvolňují automaticky.
