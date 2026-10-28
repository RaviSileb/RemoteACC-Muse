# racc-cli

CLI agent a klient RemoteACC.

```sh
racc-cli keygen --out device.key
racc-cli serve --key device.key --device-id pc1 \
  --bootstrap vps:8443 --bootstrap-cert bootstrap.der --server-name vps
racc-cli tty --target host:port --cert agent.der --server-name pc1
racc-cli file-send --target host:port --cert agent.der --server-name pc1 \
  --file a.zip --dest inbox/a.zip
racc-cli lookup --bootstrap vps:8443 --cert bootstrap.der \
  --server-name vps --device-id pc1
```

Logy na stderr, datové výstupy (JSON) na stdout. `--insecure` = TOFU
režim pro vývoj. Příklady: kořenový `README.md`, `docs/TESTOVANI.md`.
