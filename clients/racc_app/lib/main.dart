import 'package:ffi/ffi.dart';
import 'package:flutter/material.dart';

import 'login.dart';
import 'racc_ffi.dart';

void main() => runApp(const RaccApp());

class RaccApp extends StatefulWidget {
  const RaccApp({super.key});

  @override
  State<RaccApp> createState() => _RaccAppState();
}

class _RaccAppState extends State<RaccApp> {
  bool _loggedIn = false;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'RemoteACC',
      theme: ThemeData(colorSchemeSeed: Colors.indigo, useMaterial3: true),
      home: _loggedIn
          ? HomePage(onLogout: () => setState(() => _loggedIn = false))
          : LoginScreen(onLogin: () => setState(() => _loggedIn = true)),
    );
  }
}

class HomePage extends StatefulWidget {
  const HomePage({super.key, required this.onLogout});

  final VoidCallback onLogout;

  @override
  State<HomePage> createState() => _HomePageState();
}

class _HomePageState extends State<HomePage> {
  Racc? _racc;
  String? _error;
  String _version = '…';
  String _identity = '';
  final _uriController =
      TextEditingController(text: 'racc://vps.example.com:8443/muj-pc#');
  String _uriResult = '';

  @override
  void initState() {
    super.initState();
    try {
      final racc = Racc.load();
      final verPtr = racc.version();
      setState(() {
        _racc = racc;
        _version = verPtr.cast<Utf8>().toDartString();
      });
    } catch (e) {
      setState(() => _error = 'Nativní knihovna nenačtena: $e');
    }
  }

  @override
  void dispose() {
    _uriController.dispose();
    super.dispose();
  }

  void _newIdentity() {
    final racc = _racc;
    if (racc == null) return;
    final m = racc.newIdentity();
    setState(() {
      _identity = m.containsKey('error')
          ? 'Chyba: ${m['error']}'
          : 'fingerprint:\n${m['fingerprint']}\n\n'
              'secret (zkráceně): ${(m['secret_hex'] as String).substring(0, 16)}…';
    });
  }

  void _validateUri() {
    final racc = _racc;
    if (racc == null) return;
    final ok = racc.isValidUri(_uriController.text.trim());
    setState(() => _uriResult = ok ? 'URI je platné.' : 'URI je NEPLATNÉ.');
  }

  void _parseUri() {
    final racc = _racc;
    if (racc == null) return;
    final m = racc.parseUri(_uriController.text.trim());
    setState(() => _uriResult = m.entries.map((e) => '${e.key}: ${e.value}').join('\n'));
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('RemoteACC'),
        actions: [
          IconButton(
            icon: const Icon(Icons.logout),
            tooltip: 'Odhlásit',
            onPressed: widget.onLogout,
          ),
        ],
      ),
      body: SingleChildScrollView(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text('Nativní jádro: racc-core $_version',
                style: Theme.of(context).textTheme.titleMedium),
            if (_error != null)
              Padding(
                padding: const EdgeInsets.only(top: 8),
                child: Text(_error!, style: const TextStyle(color: Colors.red)),
              ),
            const SizedBox(height: 16),
            ElevatedButton(
              onPressed: _racc == null ? null : _newIdentity,
              child: const Text('Nová identita'),
            ),
            if (_identity.isNotEmpty)
              Padding(
                padding: const EdgeInsets.only(top: 8),
                child: SelectableText(_identity),
              ),
            const SizedBox(height: 16),
            TextField(
              controller: _uriController,
              decoration: const InputDecoration(
                labelText: 'racc:// URI',
                border: OutlineInputBorder(),
              ),
            ),
            const SizedBox(height: 8),
            Row(
              children: [
                Expanded(
                  child: OutlinedButton(
                    onPressed: _racc == null ? null : _validateUri,
                    child: const Text('Ověřit'),
                  ),
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: OutlinedButton(
                    onPressed: _racc == null ? null : _parseUri,
                    child: const Text('Parsovat'),
                  ),
                ),
              ],
            ),
            if (_uriResult.isNotEmpty)
              Padding(
                padding: const EdgeInsets.only(top: 8),
                child: SelectableText(_uriResult),
              ),
          ],
        ),
      ),
    );
  }
}
