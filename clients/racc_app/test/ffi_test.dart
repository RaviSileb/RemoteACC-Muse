// Hostitelský test FFI proti `racc_core` sestavené pro tento PC.
// Před spuštěním: `cargo build -p racc-core` (debug) v kořeni repa.
import 'dart:io' show Directory, File, Platform;

import 'package:ffi/ffi.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:racc_app/racc_ffi.dart';

String _hostLib() {
  // `flutter test` běží z kořene balíčku (clients/racc_app).
  final repoRoot = Directory.current.parent.parent;
  final name = Platform.isWindows
      ? 'racc_core.dll'
      : Platform.isMacOS
          ? 'libracc_core.dylib'
          : 'libracc_core.so';
  final mode = Platform.environment['RACC_LIB_MODE'] ?? 'debug';
  return '${repoRoot.path}/target/$mode/$name';
}

void main() {
  test('version + identity + uri roundtrip', () {
    final path = _hostLib();
    expect(File(path).existsSync(), isTrue,
        reason: 'Chybí $path — nejdřív: cargo build -p racc-core');
    final racc = Racc.load(path);

    final ver = racc.version().cast<Utf8>().toDartString();
    expect(ver, matches(RegExp(r'^\d+\.\d+\.\d+$')));

    final id = racc.newIdentity();
    expect(id.containsKey('error'), isFalse);
    final secret = id['secret_hex'] as String;
    final fp = id['fingerprint'] as String;
    expect(secret, matches(RegExp(r'^[0-9a-f]{64}$')));
    expect(fp, matches(RegExp(r'^[0-9a-f]{64}$')));

    final fp2 = racc.fingerprintOf(secret);
    expect(fp2['fingerprint'], equals(fp));

    final uri = 'racc://vps.example.com:8443/muj-pc#$fp';
    expect(racc.isValidUri(uri), isTrue);
    expect(racc.isValidUri('http://neplatne'), isFalse);

    final parsed = racc.parseUri(uri);
    expect(parsed['host'], equals('vps.example.com'));
    expect(parsed['port'], equals(8443));
    expect(parsed['device_id'], equals('muj-pc'));
  });
}
