# racc-av

Audio/video subsystém RemoteACC. MVP obsahuje čistou logiku bez OS
závislostí: `tiles` (dirty dlaždice 64×64), `codec` (`select_codec`
kaskáda + `FpsController`), `cursor` (client-side kurzor), `backend`
(`CaptureBackend` trait + `MockSource`).

OS zachytávání (DXGI / PipeWire / ScreenCaptureKit) přijde ve fázi 3
za feature flagy `win-dxgi`, `linux-pipewire`, `macos-sck`.
