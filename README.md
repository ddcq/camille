# Camille

Avatar 3D (Fyrox) piloté par webcam + voix. Head-tracking MediaPipe, expressions faciales, micro, TTS.

Prototype Rust.

## Stack

- [Fyrox 1.0](https://github.com/FyroxEngine/Fyrox) — moteur 3D / scène / caméra orbitale
- `nokhwa` — capture webcam
- `mediapipe` — `face_landmarker.task` → yaw/pitch tête + blendshapes
- `whisper-cpp` (`ggml-tiny.bin`) — speech-to-text
- `tts` : Piper (`fr_FR-siwis-medium.onnx`) + Kokoro (`model_quantized.onnx`)
- `rodio` — lecture audio

## Lancer

```bash
cargo run --release
```

Contrôles : molette = zoom, clavier = orbite caméra, `demo` = mode démo sans webcam.

> 🐧 **Fedora ostree ?** Suivre [docs/fedora-ostree.md](docs/fedora-ostree.md) : toolbox, webcam/micro, Piper, modèles, dépannage.

## Modèles (non versionnés, ~270 MB)

Télécharger / placer manuellement :

| Fichier | Destination |
|---|---|
| `face_landmarker.task` (MediaPipe) | racine (déjà inclus, 3,6 Mo) |
| `ggml-tiny.bin` (whisper) | `models/` |
| `ggml-silero-v6.2.0.bin` (VAD) | `models/` |
| `fr_FR-siwis-medium.onnx{,.json}` + binaire `piper` | `models/piper/` |
| `model_quantized.onnx` (kokoro) | `kokoro/models/` |
| `ff_siwis.bin` (voix kokoro) | `kokoro/voices/` |

## Structure

```
src/
  main.rs        — plugin Fyrox, scène, caméra, boucle
  face_track.rs  — webcam + landmarks → yaw/pitch
  expressions.rs — blendshapes → morphs visage
  jointtest.rs   — calibration articulations
  mic.rs / voice.rs — capture + VAD + whisper
  tts.rs         — Piper / Kokoro → rodio
assets/          — modèle Aki (.glb/.fbx) + sprites
data/            — resources.registry Fyrox
```
