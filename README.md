# Camille

Avatar 3D (Fyrox) piloté par webcam + voix. Head-tracking MediaPipe, expressions faciales, micro, TTS.

Prototype Rust.

## Stack

- [Fyrox 1.0](https://github.com/FyroxEngine/Fyrox) — moteur 3D / scène / caméra orbitale
- `nokhwa` — capture webcam
- `mediapipe` — `face_landmarker.task` → yaw/pitch tête + blendshapes
- `whisper-cpp` (`ggml-tiny.bin`) — speech-to-text
- `tts` : Piper in-process (`piper-rs`, mêmes `.onnx`) — aucune dépendance python
- `rodio` — lecture audio

## Lancer

```bash
cargo run --release
```

Contrôles : molette = zoom, clavier = orbite caméra, `demo` = mode démo sans webcam.

> 🐧 **Fedora ostree ?** Suivre [docs/fedora-ostree.md](docs/fedora-ostree.md) : toolbox, webcam/micro, Piper, modèles, dépannage.

## Modèles (non versionnés, ~140 MB)

`face_landmarker.task` (3,6 Mo) est dans git. Tout le reste va dans `/models`
(ignoré par git) — fichiers attendus par le code :

| Fichier | Taille | Utilisé par | Se procurer |
|---|---|---|---|
| `models/ggml-tiny.bin` | ~77 Mo | `voice.rs` (whisper STT) | `wget -O models/ggml-tiny.bin https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin` |
| `models/piper/fr_FR-siwis-medium.onnx` | ~63 Mo | `tts.rs` (`PIPER_MODEL`) | `wget -O models/piper/fr_FR-siwis-medium.onnx https://huggingface.co/rhasspy/piper-voices/resolve/main/fr/fr_FR/siwis/medium/fr_FR-siwis-medium.onnx` |
| `models/piper/fr_FR-siwis-medium.onnx.json` | ~5 Ko | `piper` (config voix, même nom que `.onnx` obligatoire) | `wget -O models/piper/fr_FR-siwis-medium.onnx.json https://huggingface.co/rhasspy/piper-voices/resolve/main/fr/fr_FR/siwis/medium/fr_FR-siwis-medium.onnx.json` |

Sans `ggml-tiny.bin` : `[voice] chargement whisper` en erreur, pas de commandes vocales.
Sans le `.onnx` Piper : `[tts] chargement voix piper` en erreur, Camille muette.
Test voix isolé : `cargo run --example tts_test -- "Bonjour Camille"`.

## Structure

```
src/
  main.rs        — plugin Fyrox, scène, caméra, boucle
  face_track.rs  — webcam + landmarks → yaw/pitch
  expressions.rs — blendshapes → morphs visage
  jointtest.rs   — calibration articulations
  mic.rs / voice.rs — capture + VAD + whisper
  tts.rs         — Piper → rodio
assets/          — modèle Aki (.glb/.fbx) + sprites
data/            — resources.registry Fyrox
```
