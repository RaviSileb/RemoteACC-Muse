# racc-core

Společné jádro RemoteACC: QUIC transport (`net`), kryptografie
(`crypto`: Ed25519/X25519/ChaCha20-Poly1305), Protobuf protokol
(`proto`), `config.toml` (`config`), `racc://` URI (`uri`) a C-ABI
pro GUI klienty (`ffi`).

Builduje se jako `rlib` (pro workspace) i `cdylib`
(`racc_core.dll/.so/.dylib` pro Flutter FFI).

Detaily: kořenový `README.md`, `docs/ARCHITEKTURA.md`, `docs/PROTOKOL.md`.
