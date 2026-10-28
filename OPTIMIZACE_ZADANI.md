# Optimalizace zadání RemoteACC pro realizaci

Výchozí zadání: `RemoteACC.docx`, 112 odstavců + tabulka, „Kompletní
architektonická a technická dokumentace", stav „Technický návrh".
Tento dokument zaznamenává rozhodnutí, která zužují 10měsíční vizi
(5 OS, video 60+ FPS, audio, HA cluster) na ověřitelné MVP 0.1.0,
aniž by se porušila architektura zadání.

## 1. Rozsah MVP (fáze 1–2)

**Dovnitř:** bootstrap signalizace (registr/lookup/relay mailbox),
QUIC transport, E2EE handshake + session cipher, TTY, přenos souborů,
CLI agent/klient, C-ABI základ, AV logika bez OS zachytávání.

**Ven (fáze 3+):** GUI, OS video/audio zachytávání a injekce vstupu,
STUN/TURN hole-punching, HA cluster, plný Noise handshake, auditní logy
pro ISO 27001/SOC 2. Důvody: každá z těchto položek je samostatný
projekt s vlastní maticí testů; zadání samo je řadí do fází 3–5.

## 2. Rozhodnuté volby (zadání je nechávalo otevřené)

| Volba | Rozhodnutí | Důvod |
|---|---|---|
| Rust vs C++ vs Go | **Rust** (vše včetně bootstrapd) | Varianta A „doporučeno"; jeden toolchain, jeden audit |
| Protobuf vs FlatBuffers | **Protobuf (prost)** | Žádný `protoc` v buildu (derive), stačí pro řídicí zprávy/TTY/soubory; FlatBuffers rezerva pro video |
| Flutter vs Tauri | **Flutter primárně** (stub + FFI) | Dle zadání pro 60/120 FPS mobilní GUI; C-ABI slouží oběma |
| QUIC knihovna | **quinn + tokio + rustls** | Čistý Rust, aktivní vývoj, BBR-ready congestion control |
| Kodek default | **auto → H.264 (HW) / AV1 (SW)** | Kaskáda AV1 > HEVC/H.264 > VP9/VP8 dle zadání; `select_codec` v `racc-av` |
| Relay fallback | **mailbox (store-and-forward)** | MVP náhrada paketového TURN relay; E2EE obálky, server nečte obsah |
| Certifikáty | **pinning DER + TOFU `--insecure`** | Bez PKI závislosti pro MVP; SAN musí souhlasit s `--server-name` |

## 3. Mapování na kód

- Monorepo workspace `crates/`: `racc-core` (QUIC/krypto/proto/config/FFI),
  `racc-tty`, `racc-av`, `racc-bootstrapd`, `racc-cli`.
- Protokol: `Envelope { version, session_id, kind, payload }` (prost),
  druhy Tty/Video/Audio/File/Signal/Keepalive; rámování `u32 BE délka + tělo`.
- Konfigurace: `config.toml` se sekcemi
  bootstrap/device/security/performance/services + `racc://` URI —
  přesně dle kapitoly 4 zadání (viz `config.example.toml`).
- Optimalizace ze zadání implementované už v MVP: dlaždice 64×64
  (`tiles::diff_tiles` + `merge_row_rects`), FPS regulátor „snižuj FPS,
  ne kvalitu textu" (`FpsController`), client-side kurzor (`CursorState`),
  chunkované soubory se SHA-256 a zstd, pinning + TOFU.

## 4. Co zadání neobsahovalo (doplněno / zůstává otevřené)

- **VPS:** konkrétní OS/distro, dimenzace, doména — doplněno jako
  parametrizovaný postup v `docs/VPS_DEPLOYMENT.md` (placeholder
  `vps.example.com:8443` zachován v `config.example.toml`).
- **Testy:** zadání nemělo žádné případy — dodáno 31 testů
  (jednotkové, integrační QUIC loopback, PTY, E2E přes binárky).
- **Repozitář:** licence a CI doplněny (MIT OR Apache-2.0, GitHub Actions).
- **Stále otevřené** (vyžadují rozhodnutí vlastníka): produkční doména
  a certifikáty, vlastník repozitáře, mobilní GUI framework finálně,
  certifikace/auditní požadavky.

## 5. Známá zjednodušení MVP (záměrný technický dluh)

1. Handshake je Noise-XX-*styl* (X25519 + Ed25519 podpisy ephemeral klíčů),
   nikoli plná Noise Framework implementace — rozhraní `handshake_*`
   je navrženo pro výměnu za `snow` bez změny volajících.
2. E2EE vrstva je implementována a testována (`SessionCipher`), ale CLI
   datová rovina v MVP spoléhá na QUIC/TLS + pinning; plné zapojení
   `SessionCipher` do každého streamu je úkol fáze 2 (viz ROADMAP).
3. Žádný STUN/TURN hole-punching — přímé P2P funguje tam, kam projde UDP;
   jinak relay mailbox (malé zprávy), ne turn relay proudu.
4. TTY klient je bez raw-módu terminálu (řádkové chování na některých
   konzolích); interaktivní plnohodnotný terminál vyžaduje `crossterm`.
