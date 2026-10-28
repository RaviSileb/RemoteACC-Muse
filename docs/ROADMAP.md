# Roadmap (fáze 1–5 dle zadání)

## Fáze 1 – Bootstrap + E2EE ✅ (toto MVP)

- [x] QUIC signalizace: registr, lookup, relay mailbox
- [x] Ed25519/X25519 handshake + ChaCha20-Poly1305 (`SessionCipher`)
- [x] CLI agent/klient, `racc://` URI, `config.toml`
- [x] CI matrix (win/linux/macos) + fmt/clippy/test

## Fáze 2 – TTY + soubory ✅ základ / 🔲 dokončení

- [x] PTY server/klient, chunkované soubory se SHA-256 + zstd
- [ ] Plný Noise Framework (`snow`, vzor XX) místo MVP handshake
- [ ] `SessionCipher` zapojen do všech datových streamů
- [ ] Souborový resume (pokračování od chunku N)
- [ ] Raw-mód terminálu (`crossterm`), auditní logování relací

## Fáze 3 – Desktop video/audio/input 🔲

- [ ] OS zachytávání za feature flagy (`win-dxgi`, `linux-pipewire`,
      `macos-sck`) + enkodéry AV1/HEVC/H.264
- [ ] Opus audio (WASAPI/PipeWire/CoreAudio), FEC 16–128 kbps
- [ ] Injekce vstupu (SendInput/`uinput`/CGEventPost, `uiAccess` na Win)
- [ ] FlatBuffers pro video cestu (vyhodnotit proti prost)

## Fáze 4 – Mobilní klienti + NAT traversal 🔲

- [x] Flutter kostra + FFI + APK pro S23 Ultra (`clients/racc_app`, viz `docs/ANDROID.md`)
- [ ] Plný mobilní klient (TTY, soubory, video) a iOS build
- [ ] ICE STUN/TURN hole-punching (`webrtc-rs`), TURN relay proudu
      místo mailboxu; TCP/TLS 1.3 fallback
- [ ] MediaProjection/ReplayKit, AccessibilityService vstup

## Fáze 5 – Produkce 🔲

- [ ] HA cluster bootstrap, eBPF/XDP DDoS ochrana, metriky (RTT/loss)
- [ ] Podepisování releasů pro 5 OS, aktualizace, CSR/TOFU správa
- [ ] ISO 27001 / SOC 2 auditní logy, penetrační test E2EE

## Otevřené volby pro vlastníka

1. Produkční doména/VPS parametry a vlastnictví certifikátů.
2. Vlastník GitHub repozitáře a politiky releasů.
3. ~~Flutter vs Tauri finální rozhodnutí pro desktop GUI~~ — ROZHODNUTO: Flutter
   (sdílený kód s Androidem, `clients/racc_app` + Windows platforma, viz `docs/GUI.md`).
4. Rozsah auditních logů (právní požadavek vs minimalizace dat).
