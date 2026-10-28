# racc-bootstrapd

Signalizační server RemoteACC pro VPS (QUIC/UDP, výchozí `:8443`):
registr zařízení, lookup, výpis, relay mailbox, health.

```sh
racc-bootstrapd --bind 0.0.0.0:8443 --dump-cert bootstrap.der
racc-bootstrapd --bind 0.0.0.0:8443 --cert cert.der --key key.der
```

Nasazení: `docs/VPS_DEPLOYMENT.md`, `deploy/Dockerfile`,
`deploy/bootstrapd.service`. Protokol: `docs/PROTOKOL.md`.
