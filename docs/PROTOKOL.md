# Drátový protokol RemoteACC 0.1.0

Vše běží nad QUIC (UDP; TLS 1.3, RFC 9000). Rámování na každém bidi
streamu: `u32 BE délka + tělo`, max 16 MiB (`net::MAX_FRAME`).

## 1. Signalizace (klient ↔ bootstrap)

Jeden požadavek = jeden bidi stream: klient zapíše JSON rámec,
zavře zapisovací stranu (`finish()`), server odpoví JSON rámcem.

```jsonc
// register
{ "op": "register", "device_id": "muj-pc", "fingerprint": "ab..",
  "blob_b64": "<volitelná E2EE obálka s kandidáty>" }
→ { "ok": true }

// lookup
{ "op": "lookup", "device_id": "muj-pc" }
→ { "ok": true, "device": { "device_id": "muj-pc", "fingerprint": "ab..",
    "addr": "203.0.113.7:51234", "blob_b64": null,
    "last_seen_unix": 1761650000 } }

// list → { "ok": true, "devices": ["a", "b"] }
// health → { "ok": true }

// relay mailbox (MVP náhrada TURN relay proudu)
{ "op": "send", "to": "muj-pc", "payload_b64": "<E2EE obálka, max 1 MiB>" }
→ { "ok": true }
{ "op": "poll", "device_id": "muj-pc" }
→ { "ok": true, "messages": ["<base64>", …] }   // doručí a smaže (max 100)
```

Chyby: `{ "ok": false, "error": "…" }`. Neznámá `op` → chyba.

## 2. Přímá datová rovina (klient ↔ agent)

První rámec streamu je JSON hlavička služby:

```jsonc
{ "service": "tty", "cols": 120, "rows": 30 }
{ "service": "file" }
```

Následují rámce `Envelope` (prost, `ENVELOPE_VERSION = 1`):

```text
Envelope { version: u32 = 1, session_id: bytes(16),
           kind: Tty|Video|Audio|File|Signal|Keepalive, payload: bytes }
```

- **tty**: oběma směry `TtyData { pty_id, data, resize?, eof }`.
  Klient posílá stdin + resize, server výstup PTY; `eof` ukončuje.
- **file**: klient posílá `FileChunk` v pořadí
  (`transfer_id, path, index, total_chunks, sha256, compressed, data, last`),
  pak `finish()`; server ověří SHA-256 každého chunku, zkontroluje
  relativní cestu (zákaz `..` a absolutních), zapíše do inboxu
  a odpoví jedním JSON rámcem `{ "ok": true, "path": "…" }`.

## 3. E2EE handshake (Noise-XX styl, MVP)

1. `A → B`: `HandshakeInit { identity_pub(A), ephemeral_pub(a), sign_A(a) }`
2. `B`: ověří podpis, vygeneruje `b`,
   `B → A`: `HandshakeResp { identity_pub(B), ephemeral_pub(b), sign_B(b) }`
3. Obě strany: `shared = X25519(a, b)`,
   `key = SHA-256(shared ‖ a_pub ‖ b_pub)`,
   otisky protistran z `identity_pub` (TOFU nebo allowlist).
4. Provoz: `SessionCipher` (ChaCha20-Poly1305), paket = `12B nonce ‖ ct`,
   nonce náhodný na každou zprávu.

Fáze 2: výměna za plný Noise Framework (`snow`, vzor XX) a zapojení
`SessionCipher` do všech datových streamů pod QUIC/TLS.

## 4. URI a fingerprinty

- Zařízení: `racc://vps.example.com:8443/muj-pc#<fingerprint>`,
  kde `fingerprint = hex(SHA-256(Ed25519 verifying key))`.
- Bootstrap pinning: `hex(SHA-256(cert DER))`, server ho loguje při startu.
