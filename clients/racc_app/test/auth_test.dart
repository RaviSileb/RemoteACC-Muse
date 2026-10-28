// Testy přihlašování (čistá logika, bez souborů a platformy).
import 'package:flutter_test/flutter_test.dart';
import 'package:racc_app/auth.dart';

void main() {
  test('hashPassword odpovídá nezávislému vektoru', () {
    // SHA-256("somesalt:test123") spočteno mimo Dart (.NET).
    expect(
        hashPassword('test123', 'somesalt'),
        equals(
            '22d9aa1722a3a92819b06473cf1a37f5e7df4e521dd90d2beb99f6ccd19564d3'));
  });

  test('verifyPassword porovnává správně', () {
    final hash = hashPassword('tajne-heslo', 'sul-1');
    expect(verifyPassword('tajne-heslo', 'sul-1', hash), isTrue);
    expect(verifyPassword('jine-heslo', 'sul-1', hash), isFalse);
    expect(verifyPassword('tajne-heslo', 'sul-2', hash), isFalse);
  });

  test('fromJson odmítne poškozená data', () {
    expect(AdminCredentials.fromJson({}), isNull);
    expect(AdminCredentials.fromJson({'username': 'admin'}), isNull);
    expect(
        AdminCredentials.fromJson(
            {'username': '', 'salt': 's', 'hash': 'h'}),
        isNull);
    final ok = AdminCredentials.fromJson(
        {'username': 'admin', 'salt': 's', 'hash': 'h'});
    expect(ok?.username, equals('admin'));
    expect(ok?.toJson(), equals({'username': 'admin', 'salt': 's', 'hash': 'h'}));
  });

  test('generátory mají správný tvar', () {
    expect(generatePassword(20), matches(RegExp(r'^[A-Za-z0-9]{20}$')));
    expect(generateSaltHex(16), matches(RegExp(r'^[0-9a-f]{32}$')));
  });
}
