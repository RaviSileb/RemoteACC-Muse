# RemoteACC – vzdálená správa zařízení za NAT (MVP 0.1.0)

Distribuovaná infrastruktura pro vzdálenou správu a ovládání zařízení
v privátních sítích (za NAT/CGNAT/firewallem) **bez veřejné IP a bez
port forwardingu**. Přímé P2P spojení přes QUIC (UDP), signalizace
a fallback relay přes vlastní VPS.

Zadání: `RemoteACC.docx` (kompletní architektonická dokumentace v1.0.0).
Optimalizace zadání pro realizaci: [OPTIMIZACE_ZADANI.md](OPTIMIZACE_ZADANI.md).

## Stav MVP (fáze 1–2)

| Subsystém | Stav |
|---|---|
| Bootstrap/signaling server (`racc-bootstrapd`) | ✅ registr, lookup, relay mailbox |
| E2EE handshake (Ed25519 + X25519, ChaCha20-Poly1305) | ✅ |
| QUIC transport + rámování (prost `Envelope`) | ✅ |
| TTY (PTY server + klient) | ✅ |
| Přenos souborů (chunky, SHA-256, zstd, resume-ready) | ✅ |
| CLI agent (`racc-cli`) | ✅ keygen, serve, tty, file-send, lookup, relay |
| C-ABI pro GUI/mobilní klienty | ✅ základ (verze, identita, URI) |
| AV logika (dlaždice 64×64, kodeky, FPS, kurzor) | ✅ logika; OS zachytávání ve fázi 3 |
| GUI (Flutter) | 🔲 stub + FFI kontrakt |
| Plný Noise Framework / ICE STUN-TURN hole-punching | 🔲 fáze 2–3 (viz roadmap) |

## Rychlý start (Windows / Linux / macOS)

```sh
cargo build --release
cp config.example.toml config.toml   # a upravte host/port

# 1) Bootstrap server (na VPS, viz docs/VPS_DEPLOYMENT.md)
./target/release/racc-bootstrapd --bind 0.0.0.0:8443 --dump-cert bootstrap.der

# 2) Agent na spravovaném zařízení
./target/release/racc-cli keygen --out device.key
./target/release/racc-cli serve --key device.key --device-id muj-pc \
  --bootstrap vps.example.com:8443 --bootstrap-cert bootstrap.der \
  --server-name vps.example.com --dump-cert agent.der

# 3) Klient: terminál a soubory (přímo na agenta)
./target/release/racc-cli tty --target AGENT:PORT --cert agent.der --server-name muj-pc
./target/release/racc-cli file-send --target AGENT:PORT --cert agent.der \
  --server-name muj-pc --file data.zip --dest inbox/data.zip

# 4) Dotazy na bootstrap registr / relay mailbox
./target/release/racc-cli lookup --bootstrap vps.example.com:8443 \
  --cert bootstrap.der --server-name vps.example.com --device-id muj-pc
```

Tip pro lokální vývoj: místo `--cert/--server-name` funguje `--insecure`
(TOFU). Produkční nasazení vždy s pinningem certifikátu.

## Struktura repozitáře

```text
crates/
  racc-core/        QUIC, krypto, Protobuf protokol, config, racc:// URI, C-ABI
  racc-tty/         PTY sezení (ConPTY/openpty) multiplexovaná přes QUIC
  racc-av/          dlaždice, kaskáda kodeků, FPS regulátor, kurzor, backendy
  racc-bootstrapd/  signalizační server pro VPS (+ Dockerfile, systemd unit)
  racc-cli/         CLI agent a klient
clients/
  flutter_stub/     kostra Flutter klienta + Dart FFI kontrakt
scripts/
  install.ps1       instalátor racc-cli pro Windows (PowerShell)
  install.sh        instalátor pro Linux/macOS/Termux
deploy/
  Dockerfile        multi-stage build bootstrap serveru
  bootstrapd.service systemd unit pro VPS
docs/
  ARCHITEKTURA.md   mapování zadání na kód
  PROTOKOL.md       drátový protokol (signalizace + datová rovina)
  VPS_DEPLOYMENT.md nasazení bootstrap serveru na VPS
  INSTALACE_KLIENTA.md instalace klienta (Windows/Linux/macOS/mobilní)
  TESTOVANI.md      testovací strategie a manuální smoke testy
  ROADMAP.md        fáze 1–5 a otevřené volby
.github/workflows/ci.yml       fmt + clippy + test + build (win/linux/macos)
.github/workflows/release.yml  tag v* → GitHub release s racc-cli
```

## Vývoj

```sh
cargo fmt --all
cargo clippy --workspace --all-targets   # musí být bez warningů
cargo test --workspace                   # 31 testů včetně E2E
```

Požadavky: Rust 1.77+ (edice 2021), pro fázi 3 navíc Flutter SDK.
Detaily viz [docs/TESTOVANI.md](docs/TESTOVANI.md).

## Bezpečnostní model (stručně)

- E2EE zero-trust: VPS vidí jen šifrované obálky a směrovací metadata.
- Identity: Ed25519, otisk `hex(SHA-256(verifying-key))` v `racc://` URI.
- Sezení: X25519 ECDH + podpis ephemeral klíčů identitou, šifra ChaCha20-Poly1305.
- Transport: QUIC (TLS 1.3) s pinningem certifikátu, TOFU jen pro vývoj.
- Soubory: SHA-256 na chunk, zákaz absolutních cest a `..`.

## Licence

MIT OR Apache-2.0 (viz `LICENSE-MIT`, `LICENSE-APACHE`).
