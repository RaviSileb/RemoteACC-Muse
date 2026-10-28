import 'package:flutter/material.dart';

import 'auth.dart';

/// Přihlašovací obrazovka (sdílená pro Windows i Android).
class LoginScreen extends StatefulWidget {
  const LoginScreen({super.key, required this.onLogin});

  final VoidCallback onLogin;

  @override
  State<LoginScreen> createState() => _LoginScreenState();
}

class _LoginScreenState extends State<LoginScreen> {
  final _user = TextEditingController(text: 'admin');
  final _pass = TextEditingController();
  String? _error;
  bool _busy = false;

  @override
  void dispose() {
    _user.dispose();
    _pass.dispose();
    super.dispose();
  }

  Future<void> _submit() async {
    setState(() {
      _busy = true;
      _error = null;
    });
    final creds = await loadCredentials();
    if (!mounted) return;
    if (creds == null) {
      setState(() {
        _busy = false;
        _error = 'Admin účet není nastaven.';
      });
      return;
    }
    final ok = _user.text.trim() == creds.username &&
        verifyPassword(_pass.text, creds.salt, creds.hash);
    if (!mounted) return;
    if (ok) {
      widget.onLogin();
    } else {
      setState(() {
        _busy = false;
        _error = 'Neplatné jméno nebo heslo.';
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('RemoteACC – přihlášení')),
      body: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 360),
          child: Padding(
            padding: const EdgeInsets.all(24),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                TextField(
                  controller: _user,
                  decoration: const InputDecoration(
                    labelText: 'Uživatel',
                    border: OutlineInputBorder(),
                  ),
                  enabled: !_busy,
                  onSubmitted: (_) => _submit(),
                ),
                const SizedBox(height: 12),
                TextField(
                  controller: _pass,
                  decoration: const InputDecoration(
                    labelText: 'Heslo',
                    border: OutlineInputBorder(),
                  ),
                  obscureText: true,
                  enabled: !_busy,
                  onSubmitted: (_) => _submit(),
                ),
                if (_error != null)
                  Padding(
                    padding: const EdgeInsets.only(top: 8),
                    child: Text(_error!,
                        style: const TextStyle(color: Colors.red)),
                  ),
                const SizedBox(height: 16),
                ElevatedButton(
                  onPressed: _busy ? null : _submit,
                  child: Text(_busy ? 'Ověřuji…' : 'Přihlásit'),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
