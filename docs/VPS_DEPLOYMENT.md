# Nasazení bootstrap serveru na VPS

Cíl: `racc-bootstrapd` na Linux VPS s veřejnou IPv4/IPv6, port 8443/UDP.
Zadání konkrétní VPS neuvádí — postup je parametrizovaný, placeholder
`vps.example.com:8443` nahraďte vlastní doménou/IP.

## Požadavky

- Linux (x86_64 nebo AArch64), libovolná distribuce; ~256 MB RAM stačí.
- Otevřený port **8443/UDP** (QUIC). TCP není třeba (MVP nemá TCP fallback).
- Volitelně doména s DNS A/AAAA záznamem (pro stabilní `--server-name`).

## Varianta A – Docker (doporučeno)

```sh
# Na VPS:
git clone <repo> remoteacc && cd remoteacc
docker build -f deploy/Dockerfile -t remoteacc/bootstrapd:0.1.0 .
mkdir -p /etc/remoteacc
docker run -d --name bootstrapd --restart unless-stopped \
  -p 8443:8443/udp -v /etc/remoteacc:/data \
  remoteacc/bootstrapd:0.1.0 \
  --bind 0.0.0.0:8443 --dump-cert /data/bootstrap.der
docker exec bootstrapd cat /data/bootstrap.der > bootstrap.der
docker logs bootstrapd 2>&1 | grep fingerprint   # zaznamenejte otisk!
```

`bootstrap.der` rozdistribuujte klientům (spolu s otiskem k ověření).

## Varianta B – systemd (nativní binárka)

```sh
# Build (na VPS s Rustem, nebo cross-build a scp):
cargo build --release -p racc-bootstrapd
sudo install -m755 target/release/racc-bootstrapd /usr/local/bin/
sudo cp deploy/bootstrapd.service /etc/systemd/system/
sudo mkdir -p /etc/remoteacc && sudo systemctl daemon-reload
sudo systemctl enable --now racc-bootstrapd
journalctl -u racc-bootstrapd | grep -E 'fingerprint|listening'
sudo cat /etc/remoteacc/bootstrap.der > bootstrap.der   # pro klienty
```

## Klienti

```sh
racc-cli serve --key device.key --device-id muj-pc \
  --bootstrap vps.example.com:8443 --bootstrap-cert bootstrap.der \
  --server-name vps.example.com
```

Poznámky:

- Ephemeral cert má SAN `remoteacc-bootstrap` — proto je nutné
  `--server-name` (nebo vlastní `--cert/--key` se SAN vaší domény).
- `--insecure` (TOFU) jen pro první seznámení; produkce vždy s pinningem.
- Firewall: povolit pouze 8443/UDP; SSH dle potřeby. eBPF/XDP DDoS
  ochrana a HA cluster jsou téma fáze 4–5 (viz ROADMAP).
- Zálohy: `/etc/remoteacc/bootstrap.der` (+ případné `--cert/--key`).
  Registr zařízení je in-memory (restart = prázdný registr, agenti se
  do 30 s znovu zaregistrují).
