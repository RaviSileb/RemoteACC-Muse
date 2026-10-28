# Instalace klienta (Windows / Linux / macOS / mobilní)

Klient MVP je `racc-cli`: umí roli **agenta** (`serve` na spravovaném
zařízení) i roli **klienta** (`tty`, `file-send`, `lookup`, `relay-*`).
Grafické GUI (Flutter) vznikne ve fázi 4 — viz
[clients/flutter_stub](../clients/flutter_stub/README.md).

V celém návodu nahraďte `vps.example.com:8443` adresou vlastního
bootstrap serveru ([nasazení na VPS](VPS_DEPLOYMENT.md)).

## Varianta 1 – release (doporučeno)

### Windows (PowerShell)

```powershell
irm https://raw.githubusercontent.com/RaviSileb/RemoteACC-Muse/main/scripts/install.ps1 | iex
```

Skript stáhne `racc-cli` z nejnovějšího release, uloží do
`~\.local\bin`, přidá do uživatelské PATH a ověří spuštění.
Jiná verze: `-Version v0.1.0`. Bez změny PATH: `-NoPathUpdate`.

### Linux / macOS

```sh
curl -fsSL https://raw.githubusercontent.com/RaviSileb/RemoteACC-Muse/main/scripts/install.sh | bash
```

Instaluje do `~/.local/bin` (`INSTALL_DIR`), verzi volí
`... | VERSION=v0.1.0 bash` (výchozí `latest`). Na macOS skript
sám maže karanténu. Pro platformy bez hotové binárky (např. Intel
Mac, Raspberry Pi) automaticky sestaví ze zdrojů (vyžaduje Rust).

### Termux (Android)

Stejný příkaz jako pro Linux — skript Termux pozná a sestaví
`racc-cli` ze zdrojů (`pkg install rust git clang`, instalace do
`$PREFIX/bin`). První instalace trvá několik minut.

## Varianta 2 – hotové binárky z CI

CI staví `racc-cli` pro Windows, Linux i macOS při každém push/PR
([ci.yml](../.github/workflows/ci.yml), job `build-release`).
Artefakty se jmenují `remoteacc-windows-latest`, `remoteacc-ubuntu-latest`,
`remoteacc-macos-latest` (GitHub je maže po expiraci, typicky 90 dní).

```sh
# ID posledního úspěšného CI běhu na main
RUN=$(gh run list -R RaviSileb/RemoteACC-Muse --branch main \
  --workflow ci --status success --limit 1 --json databaseId -q '.[0].databaseId')

# Dle platformy (název artefaktu viz výše)
gh run download -R RaviSileb/RemoteACC-Muse $RUN -n remoteacc-ubuntu-latest -D racc-bin
```

Binárka je `racc-cli` (`racc-cli.exe` na Windows). Ověření:

```sh
./racc-bin/racc-cli --help
```

## Varianta 3 – sestavení ze zdrojů

Požadavky společné: Git a Rust 1.77+ (edice 2021):

```sh
git clone https://github.com/RaviSileb/RemoteACC-Muse.git
cd RemoteACC-Muse
cargo build --release -p racc-cli
# výstup: target/release/racc-cli (.exe na Windows)
```

### Windows 10/11 (x86_64)

1. Nainstalujte Rust přes [rustup](https://rustup.rs/) (vyžaduje
   MSVC linker — rustup nabídne instalaci VS Build Tools).
2. Sestavte dle postupu výše, poté volitelně do PATH:
   ```powershell
   Copy-Item target\release\racc-cli.exe $env:USERPROFILE\.cargo\bin\
   ```
3. Terminál/PTY na agentovi běží přes ConPTY (součást Windows 10+).
4. Firewall: klient potřebuje **odchozí** UDP/8443 na bootstrap;
   příchozí porty nejsou potřeba (i agent za NAT navazuje spojení ven).

### Linux – Debian/Ubuntu (x86_64)

```sh
sudo apt install build-essential pkg-config libssl-dev
cargo build --release -p racc-cli
install -m755 target/release/racc-cli ~/.local/bin/   # ~/.local/bin musí být v PATH
```

Agent jako služba (automatický start, `serve`):

```ini
# ~/.config/systemd/user/racc-agent.service
[Unit]
Description=RemoteACC agent
After=network-online.target
Wants=network-online.target

[Service]
ExecStart=%h/.local/bin/racc-cli serve --key %h/.config/remoteacc/device.key \
  --device-id muj-pc --bootstrap vps.example.com:8443 \
  --bootstrap-cert %h/.config/remoteacc/bootstrap.der \
  --server-name vps.example.com --inbox %h/.config/remoteacc/inbox
Restart=on-failure

[Install]
WantedBy=default.target
```

```sh
mkdir -p ~/.config/remoteacc
# device.key + bootstrap.der zkopírujte dle kap. „První spuštění"
systemctl --user enable --now racc-agent
```

Na Fedoře/RHEL nahraďte `apt install` za
`sudo dnf install gcc pkg-config openssl-devel`.

### macOS 13+ (Apple Silicon / Intel)

```sh
xcode-select --install   # kompilátor (jen pro sestavení ze zdrojů)
cargo build --release -p racc-cli
install -m755 target/release/racc-cli /usr/local/bin/
```

Stažené binárky z CI nejsou notářsky podepsané — po stažení smažte
karanténu, jinak Gatekeeper spuštění odmítne:

```sh
xattr -d com.apple.quarantine racc-cli
```

## První spuštění (všechny desktopové platformy)

```sh
# 1) Identita zařízení (hex secret) + kontrola otisku
racc-cli keygen --out device.key
racc-cli fingerprint --key device.key

# 2) Agent: registrace na bootstrap + přímý QUIC server
racc-cli serve --key device.key --device-id muj-pc \
  --bootstrap vps.example.com:8443 --bootstrap-cert bootstrap.der \
  --server-name vps.example.com --dump-cert agent.der

# 3) Klient z jiného PC: dotaz do registru, terminál, soubor
racc-cli lookup --bootstrap vps.example.com:8443 \
  --cert bootstrap.der --server-name vps.example.com --device-id muj-pc
racc-cli tty --target AGENT:PORT --cert agent.der --server-name muj-pc
racc-cli file-send --target AGENT:PORT --cert agent.der \
  --server-name muj-pc --file data.zip --dest inbox/data.zip
```

- `bootstrap.der` získáte z VPS ([VPS_DEPLOYMENT.md](VPS_DEPLOYMENT.md));
  otisk vždy ověřte proti `fingerprint` v logu serveru.
- Ephemeral cert serveru má SAN `remoteacc-bootstrap` — pak musí
  `--server-name remoteacc-bootstrap`. S vlastním certem se SAN vaší
  domény použijte `--server-name vps.example.com`.
- `--insecure` (TOFU) jen pro lokální vývoj, nikdy do produkce.

## Konfigurace `config.toml`

Volitelná (CLI přepínače mají přednost). Vzor:
[config.example.toml](../config.example.toml) → zkopírovat jako `config.toml`:

- `[bootstrap]` – `host`, `port`, `pinned_fingerprint` (SHA-256 TLS
  certifikátu serveru; prázdné = TOFU při prvním připojení).
- `[device]` – `id` (prázdné = vygeneruje se), `name`.
- `[security]` – `pairing_pin` (nikdy do repozitáře!),
  `trusted_fingerprints`, `tofu`.
- `[performance]` / `[services]` – `max_fps`, `codec`, `hw_accel`,
  `direct_p2p_only`; které služby agent nabízí (`tty`, `file_transfer`,
  `video`, `audio`).

## Android (nativní aplikace) / iOS (výhled)

Pro Android existuje Flutter aplikace [clients/racc_app](../clients/racc_app/README.md)
— funkční kostra (verze jádra, identita, `racc://` URI) s APK pro Galaxy S23 Ultra.
Sestavení, instalace přes `adb` a vývojový běh: [docs/ANDROID.md](ANDROID.md).
Plný klient (TTY, soubory, video) je fáze 4.

iOS: zatím jen FFI kontrakt (`racc-core` jako `cdylib` + Dart vazby);
build přes cargo-lipo / xcframework dle [ROADMAP](ROADMAP.md).

Platformní omezení (dle zadání): na Androidu běží agent i klient,
na iOS **jen klient** (systém nedovolí PTY server ani zachytávání
obrazovky na pozadí). `device.key` se na mobilu negeneruje příkazovou
řádkou, ale prvním startem aplikace (secure storage).

## Řešení problémů

| Příznak | Příčina / oprava |
|---|---|
| `error: linker 'cc' not found` | chybí kompilátor (Linux: `build-essential`, macOS: `xcode-select --install`, Windows: VS Build Tools) |
| `lookup: unknown device` | agent neběží nebo se ještě nezaregistroval (registr je in-memory, po restartu serveru se agenti hlásí do ~30 s) |
| TLS chyba / špatný SAN | `--server-name` musí souhlasit se SAN certifikátu (`remoteacc-bootstrap` vs. vlastní doména) |
| macOS „cannot be opened" | `xattr -d com.apple.quarantine racc-cli` |
| `connection lost` při `file-send` | známý ojedinělý flake pod zátěží — zopakujte přenos |
