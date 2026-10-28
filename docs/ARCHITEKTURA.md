# Architektura implementace (mapování zadání → kód)

Vrstvy dle zadání (kap. 2–3) a jejich umístění v repozitáři:

```text
┌─────────────────────────────────────────────────────────────┐
│ Klient A (racc-cli / Flutter)         Klient B (racc-cli)   │
│  TTY ◄── racc-tty::LocalPty                                   │
│  Soubory ◄── racc-core::net::{chunk_bytes, reassemble}        │
│  AV logika ◄── racc-av::{tiles, codec, cursor, backend}      │
│  E2EE ◄── racc-core::crypto::{Identity, handshake_*,        │
│                                SessionCipher}               │
│  QUIC ◄── racc-core::net::{server_endpoint, client_endpoint,│
│                             write_frame, read_frame}        │
└──────────────┬──────────────────────────────┬───────────────┘
               │ P2P UDP (QUIC)               │ fallback relay
               │                              │ mailbox (E2EE)
┌──────────────▼──────────────────────────────▼───────────────┐
│ VPS: racc-bootstrapd (tokio+quinn)                          │
│   register / lookup / list / send / poll / health           │
│   vidí jen šifrované obálky + směrovací metadata            │
└─────────────────────────────────────────────────────────────┘
```

## Crate `racc-core`

- `config.rs` – `Config` + sekce, load/save TOML, validace.
- `uri.rs` – `RaccUri` (`racc://host:port/device#fingerprint`).
- `crypto.rs` – `Identity` (Ed25519), `Ephemeral` (X25519),
  `handshake_initiate/respond/finish`, `derive_session_key`,
  `SessionCipher` (ChaCha20-Poly1305, náhodný nonce na zprávu).
- `proto.rs` – prost zprávy: `Envelope`, `TtyData`, `FileChunk`,
  `SignalMsg`, `VideoFrame`, `AudioFrame`; `ENVELOPE_VERSION = 1`.
- `net.rs` – `generate_self_signed`, `server_endpoint`,
  `client_endpoint` (pinning) / `client_endpoint_insecure` (TOFU),
  `write_frame`/`read_frame` (u32 BE + tělo, max 16 MiB),
  `chunk_bytes`/`reassemble` (SHA-256, zstd), `check_rel_path`.
- `ffi.rs` – C-ABI: `racc_version`, `racc_identity_generate`,
  `racc_fingerprint_of`, `racc_uri_parse`, `racc_uri_is_valid`,
  `racc_free_string`. Chyby jako JSON `{"error": …}`.

## Crate `racc-tty`

`LocalPty::spawn(shell, size)` → `(LocalPty, Receiver<PtyEvent>)`.
Čtení na blokujícím vlákně (neblokuje tokio), `write`/`resize`/`kill`.
Backend `portable-pty` = ConPTY na Windows, `forkpty` na unixu.

## Crate `racc-av` (MVP: logika; zachytávání fáze 3)

- `tiles` – `diff_tiles` (64×64, porovnání po 8 B), `merge_row_rects`.
- `codec` – `Codec`, `select_codec("auto"|…, hw)` (H.264 při HW, jinak AV1).
- `codec::FpsController` – multiplikativní pokles při ztrátách/RTT,
  aditivní návrat; minimum 5 FPS.
- `cursor::CursorState` – client-side kurzor, `update` hlásí změny.
- `backend` – `CaptureBackend` trait, `MockSource` (testy),
  `BackendKind::native()` za feature flagy `win-dxgi`,
  `linux-pipewire`, `macos-sck`.

## Crate `racc-bootstrapd`

Stav `State { devices, mailbox }`, operace nad JSON rámci na QUIC
bidi streamech. Detaily protokolu viz `PROTOKOL.md`. Certifikát:
`--cert/--key` (DER) nebo ephemeral self-signed + `--dump-cert`
pro pinning klientů; otisk se loguje při startu.

## Crate `racc-cli`

`keygen` (0600 na unixu), `fingerprint`, `serve` (registrační smyčka
30 s + přímý QUIC server pro `tty`/`file` služby), `tty` (obálky
`TtyData` oběma směry), `file-send` (chunky → `finish()` → JSON
potvrzení), `lookup`, `relay-send`, `relay-poll`. TLS jméno:
`--server-name`, jinak host z adresy (musí souhlasit se SAN certu).
Logy jdou na stderr, datové výstupy (JSON) na stdout.

## Důležitá QUIC pravidla (ověřená testy)

1. Drop poslední `Connection` = okamžité CONNECTION_CLOSE — server proto
   drží spojení, dokud klient nedočte odpověď (test `transport.rs`).
2. `Endpoint::wait_idle` se nevrací, dokud protistrana nezavře —
   klienti proto volají explicitní `conn.close()` (jinak ~30 s vis).
3. `finish()` ukončuje jen zapisovací stranu; čtení odpovědi pak
   stále funguje (použito u file-send i bootstrap RPC).
