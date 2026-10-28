// Lokální přihlašování k admin účtu.
//
// Brána aplikace, ne serverová autentizace: heslo se ověřuje proti
// lokálně uloženému hashi (sůl + SHA-256). Windows čte
// `%APPDATA%/RemoteACC/admin.json` (vytváří instalátor), Android používá
// hodnoty z `--dart-define=ADMIN_SALT/ADMIN_HASH` při buildu.
import 'dart:convert';
import 'dart:io';
import 'dart:math';

import 'package:crypto/crypto.dart';

class AdminCredentials {
  AdminCredentials(
      {required this.username, required this.salt, required this.hash});

  final String username;
  final String salt;
  final String hash;

  Map<String, String> toJson() =>
      {'username': username, 'salt': salt, 'hash': hash};

  static AdminCredentials? fromJson(Map<String, dynamic> json) {
    final u = json['username'], s = json['salt'], h = json['hash'];
    if (u is! String || s is! String || h is! String) return null;
    if (u.isEmpty || s.isEmpty || h.isEmpty) return null;
    return AdminCredentials(username: u, salt: s, hash: h);
  }
}

/// SHA-256 hex z `"$salt:$password"` (UTF-8). Stejný postup používá instalátor.
String hashPassword(String password, String salt) =>
    sha256.convert(utf8.encode('$salt:$password')).toString();

/// Porovná heslo s uloženým hashem.
bool verifyPassword(String password, String salt, String hash) =>
    hashPassword(password, salt) == hash;

String _randomFrom(String alphabet, int length, Random rng) => List.generate(
    length, (_) => alphabet[rng.nextInt(alphabet.length)]).join();

/// Náhodné heslo (písmena + číslice, bezpečné i pro `adb shell input text`).
String generatePassword([int length = 20]) => _randomFrom(
    'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789',
    length,
    Random.secure());

/// Náhodná sůl jako hex řetězec (`bytes` bajtů).
String generateSaltHex([int bytes = 16]) {
  final rng = Random.secure();
  return List.generate(bytes, (_) => rng.nextInt(256))
      .map((b) => b.toRadixString(16).padLeft(2, '0'))
      .join();
}

/// Windows: `%APPDATA%/RemoteACC/admin.json` (vytváří instalátor).
/// Chybějící/poškozený soubor → null.
Future<AdminCredentials?> loadWindowsCredentials() async {
  final appData = Platform.environment['APPDATA'];
  if (appData == null) return null;
  final file = File('$appData/RemoteACC/admin.json');
  try {
    final text = await file.readAsString();
    final json = jsonDecode(text);
    if (json is! Map<String, dynamic>) return null;
    return AdminCredentials.fromJson(json);
  } catch (_) {
    return null;
  }
}

/// Kredence pro aktuální platformu, nebo null když nejsou nastaveny.
Future<AdminCredentials?> loadCredentials() async {
  if (Platform.isWindows) return loadWindowsCredentials();
  if (Platform.isAndroid) {
    const salt = String.fromEnvironment('ADMIN_SALT');
    const hash = String.fromEnvironment('ADMIN_HASH');
    if (salt.isEmpty || hash.isEmpty) return null;
    return AdminCredentials(username: 'admin', salt: salt, hash: hash);
  }
  return null;
}
