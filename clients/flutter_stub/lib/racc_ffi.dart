/// Dart FFI vazby na `racc-core` (cdylib) – kostra pro fázi 4.
///
/// Odpovídá `crates/racc-core/src/ffi.rs`. Všechny `Pointer<Char>` výsledky
/// je nutné uvolnit přes [Racc.free].
library racc_ffi;

import 'dart:convert' show json, utf8;
import 'dart:ffi' as ffi;
import 'dart:io' show Platform;
import 'package:ffi/ffi.dart';

typedef _VersionC = ffi.Pointer<ffi.Char> Function();
typedef _VersionDart = ffi.Pointer<ffi.Char> Function();
typedef _GenC = ffi.Pointer<ffi.Char> Function();
typedef _GenDart = ffi.Pointer<ffi.Char> Function();
typedef _StrInC = ffi.Pointer<ffi.Char> Function(ffi.Pointer<ffi.Char>);
typedef _StrInDart = ffi.Pointer<ffi.Char> Function(ffi.Pointer<ffi.Char>);
typedef _FreeC = ffi.Void Function(ffi.Pointer<ffi.Char>);
typedef _FreeDart = void Function(ffi.Pointer<ffi.Char>);
typedef _ValidC = ffi.Int32 Function(ffi.Pointer<ffi.Uint8>, ffi.Size);
typedef _ValidDart = int Function(ffi.Pointer<ffi.Uint8>, int);

/// Tenka obálka nad dynamickou knihovnou `racc_core`.
class Racc {
  Racc._(this._lib)
      : version = _lib.lookupFunction<_VersionC, _VersionDart>('racc_version'),
        identityGenerate = _lib.lookupFunction<_GenC, _GenDart>(
            'racc_identity_generate'),
        _fingerprintOf = _lib.lookupFunction<_StrInC, _StrInDart>(
            'racc_fingerprint_of'),
        _uriParse =
            _lib.lookupFunction<_StrInC, _StrInDart>('racc_uri_parse'),
        _free =
            _lib.lookupFunction<_FreeC, _FreeDart>('racc_free_string'),
        _uriIsValid = _lib.lookupFunction<_ValidC, _ValidDart>(
            'racc_uri_is_valid');

  final ffi.DynamicLibrary _lib;
  final _VersionDart version;
  final _GenDart identityGenerate;
  final _StrInDart _fingerprintOf;
  final _StrInDart _uriParse;
  final _FreeDart _free;
  final _ValidDart _uriIsValid;

  /// Načte knihovnu pro aktuální platformu.
  factory Racc.load([String? path]) {
    final resolved = path ?? _defaultName;
    return Racc._(ffi.DynamicLibrary.open(resolved));
  }

  static String get _defaultName {
    if (Platform.isAndroid) return 'libracc_core.so';
    if (Platform.isIOS) return 'racc_core.framework/racc_core';
    if (Platform.isWindows) return 'racc_core.dll';
    if (Platform.isMacOS) return 'libracc_core.dylib';
    return 'libracc_core.so';
  }

  String _takeString(ffi.Pointer<ffi.Char> ptr) {
    try {
      return ptr.cast<Utf8>().toDartString();
    } finally {
      _free(ptr);
    }
  }

  /// `{secret_hex, fingerprint}` nebo `{error}`.
  Map<String, dynamic> newIdentity() =>
      json.decode(_takeString(identityGenerate())) as Map<String, dynamic>;

  /// `{fingerprint}` nebo `{error}`.
  Map<String, dynamic> fingerprintOf(String secretHex) {
    final arg = secretHex.toNativeUtf8();
    try {
      return json.decode(_takeString(_fingerprintOf(arg.cast()))) as Map<String, dynamic>;
    } finally {
      calloc.free(arg);
    }
  }

  /// `{host, port, device_id, fingerprint?}` nebo `{error}`.
  Map<String, dynamic> parseUri(String uri) {
    final arg = uri.toNativeUtf8();
    try {
      return json.decode(_takeString(_uriParse(arg.cast()))) as Map<String, dynamic>;
    } finally {
      calloc.free(arg);
    }
  }

  /// Rychlá validace `racc://` URI bez alokace výsledku.
  bool isValidUri(String uri) {
    final bytes = utf8.encode(uri);
    final ptr = calloc<ffi.Uint8>(bytes.length);
    try {
      ptr.asTypedList(bytes.length).setAll(0, bytes);
      return _uriIsValid(ptr, bytes.length) == 1;
    } finally {
      calloc.free(ptr);
    }
  }
}
