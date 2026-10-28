# Testování

Zadání testovací případy neobsahovalo — strategie níže je součástí MVP.

## Automatické testy (`cargo test --workspace`, 31 testů)

| Vrstva | Kde | Co |
|---|---|---|
| Jednotkové (16) | `racc-core` | config, `racc://` URI, Ed25519/X25519/handshake, cipher+tamper, prost obálky, chunky+SHA-256/zstd, cesty |
| Jednotkové (9) | `racc-av` | dlaždice (prázdné/1px/okraje), merge, kodeky, FPS pokles/návrat/minimum, kurzor, mock zdroj |
| Jednotkové (2) | `racc-tty` | spawn shellu, echo markeru přes PTY (ConPTY/`forkpty`) |
| Integrační (2) | `racc-core/tests/transport.rs` | QUIC loopback ping-pong, handshake + šifrovaná obálka |
| E2E (1) | `racc-bootstrapd/tests/e2e.rs` | skutečná binárka: register/lookup/miss/send/poll/prázdný poll |
| E2E (1) | `racc-cli/tests/e2e.rs` | keygen → serve → file-send 700 KB (3 chunky) → obsah shodný; `../` odmítnut |

Pravidla: `cargo fmt --check` a `cargo clippy --workspace --all-targets`
bez warningů (vynucuje CI). E2E testy bindují `127.0.0.1:0`, žádné
konflikty portů, žádné externí závislosti.

## Manuální smoke testy (TTY interaktivně)

```sh
# Terminál 1 – agent:
racc-cli keygen --out a.key
racc-cli serve --key a.key --device-id pc1 --bootstrap 127.0.0.1:9 \
  --insecure --bind 127.0.0.1:19001 --dump-cert a.der --inbox inbox1
# → poznamenejte si port z "direct QUIC on …"

# Terminál 2 – vzdálený shell (ukončení: Ctrl-C / exit + EOF):
echo whoami | racc-cli tty --target 127.0.0.1:19001 \
  --cert a.der --server-name pc1

# Terminál 2 – soubor tam a kontrola:
echo data > f.txt
racc-cli file-send --target 127.0.0.1:19001 --cert a.der \
  --server-name pc1 --file f.txt --dest d/f.txt
cat inbox1/d/f.txt   # v terminálu 1
```

Pozn.: TTY klient je bez raw-módu (fáze 2: `crossterm`), proto je
interaktivní demo přes pipe; plná interaktivita funguje nejlépe
s přesměrovaným vstupem.

## Fáze 2+ (plán)

- `snow` Noise-XX interop testy proti referenční implementaci.
- Souborový resume test (přerušený přenos + pokračování od chunku N).
- Zátěž: ztrátovost/RTT emulace pro `FpsController` a BBR.
- `flutter test` pro GUI klienta, E2E P2P přes 2 agenty za NAT (CI job
  se dvěma kontejnery).
