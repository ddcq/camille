# Camille sous Linux — installer, lancer, développer

Guide testé pour **Ubuntu 24.04 LTS** (adaptable Debian / Fedora / Arch, voir § 9).
Couvre une machine Linux fraîche → Camille qui tourne → boucle de dev.

## 1. Ce que fait Camille (rappel utile pour le dev)

- Fenêtre 3D **Fyrox 1.0** : avatar Aki (`assets/aki.glb`), caméra orbitale.
- Thread webcam (**nokhwa** V4L2) → **MediaPipe FaceLandmarker** (`face_landmarker.task`) → yaw/pitch tête (`src/face_track.rs`).
- Thread micro (**cpal** via `mic.rs`, 16 kHz mono, ring buffer 8 s) → **whisper-cpp** (`models/ggml-tiny.bin`, `src/voice.rs`) → wake word `camille` → commande → réponse.
- TTS **Piper** via `python3 -m piper` (`models/piper/fr_FR-siwis-medium.onnx`) → lecture **rodio** (`src/tts.rs`).

Logs au démarrage : `[face.track]`, `[voice]`, `[tts]`.

## 2. Prérequis système (Ubuntu / Debian)

```bash
sudo apt update
sudo apt install -y \
  build-essential cmake pkg-config git curl wget \
  libssl-dev \
  libxcb-shape0-dev libxcb-xfixes0-dev libxcb1-dev libxkbcommon-dev \
  libgl1-mesa-dev libx11-dev libxi-dev \
  libasound2-dev libudev-dev \
  libv4l-dev v4l-utils \
  python3 python3-pip python3-venv \
  pipewire pipewire-pulse pavucontrol
```

Détail par dépendance :

| Paquet | Pourquoi |
|---|---|
| `build-essential`, `cmake` | `whisper-cpp-plus` compile whisper.cpp en C++ au premier `cargo build` |
| `pkg-config`, `libssl-dev` | dépendances TLS de l'écosystème Cargo |
| `libxcb*`, `libxkbcommon-dev`, `libxi-dev`, `libgl1-mesa-dev` | fenêtre + contexte OpenGL Fyrox/winit |
| `libasound2-dev`, `libudev-dev` | `cpal`/`rodio` : capture micro + lecture HP |
| `libv4l-dev`, `v4l-utils` | backend V4L2 de `nokhwa` |
| `python3`, `pip` | TTS Piper appelé via `python3 -m piper` (voir § 4) |
| `pavucontrol` / `pipewire` | vérifier micro + sortie par défaut |

Vérifier webcam + micro avant de compiler :

```bash
v4l2-ctl --list-devices        # doit lister /dev/video0
ffplay /dev/video0 2>/dev/null || mpv av://v4l2:/dev/video0
pactl list short sources       # micro visible
pactl list short sinks         # sortie audio visible
```

Si `/dev/video0` absent : brancher webcam, puis `sudo usermod -aG video $USER` + reconnexion.
Si pas de micro : `pavucontrol` → onglet Entrée → choisir le bon périphérique.

## 3. Rust (via rustup, pas le paquet apt)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
rustc --version   # viser stable ≥ 1.80
```

## 4. Piper TTS (Python)

`src/tts.rs` lance `python3 -m piper --model models/piper/fr_FR-siwis-medium.onnx --output_raw`.
Il faut donc le module `piper` disponible pour le **même** `python3` :

```bash
python3 -m pip install --user --break-system-packages piper-tts
python3 -m piper --help   # doit afficher l'aide, pas "No module named piper"
```

Alternative propre (venv) — dans ce cas lancer Camille avec le venv activé :

```bash
python3 -m venv ~/.venvs/camille
~/.venvs/camille/bin/pip install piper-tts
source ~/.venvs/camille/bin/activate
```

## 5. Cloner + modèles

```bash
git clone https://github.com/ddcq/camille.git
cd camille
```

Les modèles lourds (~270 Mo) ne sont **pas** dans git (voir `.gitignore`).
Arborescence attendue après installation :

```
camille/
  face_landmarker.task                  # 3,6 Mo, déjà dans git
  models/
    ggml-tiny.bin                       # whisper STT (~75 Mo)
    ggml-silero-v6.2.0.bin              # VAD (optionnel, ~1 Mo)
    piper/fr_FR-siwis-medium.onnx       # voix Piper (~60 Mo)
    piper/fr_FR-siwis-medium.onnx.json
  kokoro/
    models/model_quantized.onnx         # TTS kokoro (89 Mo, optionnel pour l'instant)
    voices/ff_siwis.bin
```

Téléchargements :

```bash
mkdir -p models/piper kokoro/models kokoro/voices

# 1. MediaPipe FaceLandmarker — déjà dans git, sinon :
wget -O face_landmarker.task \
  https://storage.googleapis.com/mediapipe-models/face_landmarker/face_landmarker/float16/1/face_landmarker.task

# 2. Whisper tiny (fr compris) :
wget -O models/ggml-tiny.bin \
  https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin

# 3. VAD Silero (optionnel) :
wget -O models/ggml-silero-v6.2.0.bin \
  https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-silero-v6.2.0.bin

# 4. Voix Piper fr_FR siwis medium :
wget -O models/piper/fr_FR-siwis-medium.onnx \
  https://huggingface.co/rhasspy/piper-voices/resolve/main/fr/fr_FR/siwis/medium/fr_FR-siwis-medium.onnx
wget -O models/piper/fr_FR-siwis-medium.onnx.json \
  https://huggingface.co/rhasspy/piper-voices/resolve/main/fr/fr_FR/siwis/medium/fr_FR-siwis-medium.onnx.json

# 5. Kokoro : pas d'URL publique stable — copier depuis la machine macOS d'origine :
#    scp mac:proto-fyrox/kokoro/models/model_quantized.onnx kokoro/models/
#    scp mac:proto-fyrox/kokoro/voices/ff_siwis.bin kokoro/voices/
#    (Le TTS actif est Piper ; Kokoro manquant ne bloque pas le lancement.)
```

Test Piper de bout en bout :

```bash
echo "Bonjour Camille" | python3 -m piper \
  --model models/piper/fr_FR-siwis-medium.onnx --output_raw \
  | aplay -r 22050 -f S16_LE -t raw -c 1
# doit parler. Sinon : modèle absent, module piper manquant, ou pas de sortie audio.
```

## 6. Compiler + lancer

```bash
cargo run --release
```

- Premier build : 5–15 min (whisper.cpp + Fyrox). Les suivants sont incrémentaux.
- Dev rapide (itérations) : `cargo run` (profil dev, `opt-level = 1` déjà réglé dans `Cargo.toml`).
- Vérifications avant commit :

```bash
cargo fmt --check
cargo clippy -- -D warnings
```

Sortie attendue dans le terminal :

```
[face.track] caméra ouverte
[face.track] landmarker prêt
[face.track] visage détecté (cx=0.48 cy=0.51)
[voice] entendu (rms=0.0123): «...»
```

Dire **`Camille, bonjour`** → réponse TTS attendue (`[voice] wake détecté`, `[tts] lecture`).

Contrôles : molette = zoom, clavier = orbite caméra.

### Lancer sans webcam / sans micro (dev pur 3D)

- Pas de webcam : `[face.track] ERREUR: ouverture caméra` — la fenêtre 3D reste utilisable, tête immobile.
- Pas de micro : `aucun micro par défaut` via `mic.rs` — le reste tourne.
- Astuce : travailler la scène avec `cargo run` et brancher webcam/micro seulement pour tester `face_track` / `voice`.

## 7. Continuer à programmer — où toucher

```
src/main.rs        Plugin Fyrox, scène, caméra, boucle. Point d'entrée des features.
src/face_track.rs  Thread V4L2 + MediaPipe → FaceState{yaw,pitch}. Constante GAIN=1.2.
src/expressions.rs Morphs visage (Joy/Angry/Sorrow/Surprised/Fun) + visèmes.
src/jointtest.rs   Calibration articulations.
src/mic.rs         Capture cpal 16 kHz, ring buffer. API: start/window/pos/since.
src/voice.rs       Wake word + whisper + brain(). Constantes WIN_SECS/TICK_MS.
src/tts.rs         File TTS → synth_piper() → rodio. PIPER_MODEL/PIPER_RATE.
```

Recettes courantes :

**Ajouter une commande vocale** → `src/voice.rs`, fonction `brain()` :

```rust
} else if c.contains("souris") {
    "Bien sûr, je souris.".to_string()
}
```

Puis `cargo run` et dire `Camille, souris`.

**Ajouter une émotion** → `src/expressions.rs`, enum `Emotion::morphs()` (noms `Fcl_*` = morph targets du `.glb` Aki). Vérifier les noms exacts dans le modèle avant d'inventer.

**Régler le head-tracking** → `src/face_track.rs` : `GAIN`, `EYE_LEFT/RIGHT_OUTER`, lissage `TRACK_SMOOTH` dans `main.rs`.

**Changer de voix Piper** → télécharger un autre `.onnx` fr (ex. `gilles-low`), mettre à jour `PIPER_MODEL` + `PIPER_RATE` (22050 Hz pour siwis-medium, vérifier dans le `.onnx.json`).

Boucle de dev conseillée :

```bash
cargo check        # rapide, à chaque edit
cargo clippy
cargo run          # test 3D seul
cargo run --release  # test voix temps réel (whisper trop lent en debug)
```

Debug ciblé avec logs existants : `RUST_LOG` non utilisé, se fier aux `println!` `[face.track]/[voice]/[tts]`.

## 8. Dépannage

| Symptôme | Cause probable | Fix |
|---|---|---|
| `ouverture caméra` / `open_stream` | pas de `/dev/video0`, webcam prise par un autre soft, Flatpak sans accès | `v4l2-ctl --list-devices`, fermer navigateur/teams, groupe `video`, Wayland : tester sous X11 (`QT_QPA_PLATFORM=xcb` ne concerne que Qt — pour winit/Fyrox forcer `WINIT_UNIX_BACKEND=x11` si besoin) |
| `aucun micro par défaut` | entrée désactivée dans PipeWire | `pavucontrol` → Entrée, `pactl list short sources`, `arecord -l` |
| `spawn piper` / `No module named piper` | `piper-tts` installé pour un autre python | `python3 -m piper --help`, réinstaller avec le bon `python3`, ou activer le venv avant `cargo run` |
| `chargement whisper` | `models/ggml-tiny.bin` absent/corrompu | re-`wget` (§ 5), `ls -lh models/` (~75 Mo) |
| `libasound.so` / `libudev.so` manquant au link | `libasound2-dev` / `libudev-dev` absents | réinstaller § 2 |
| `xcb` / `xkbcommon` / `GL` manquant | deps fenêtre Fyrox absentes | réinstaller § 2, driver GPU proprio si VM sans 3D (`LIBGL_ALWAYS_SOFTWARE=1` en dépannage, lent) |
| `cmake not found` / erreur C++ au build | `cmake`/`g++` absents (whisper-cpp-plus) | `sudo apt install cmake g++` puis `cargo clean -p whisper-cpp-plus` |
| Whisper très lent | build debug | utiliser `--release` pour tout test voix |
| `face_landmarker.task` introuvable | lancé depuis un autre dossier | toujours `cargo run` depuis la racine du repo (chemin relatif `"face_landmarker.task"` dans `face_track.rs`) |
| Son saccadé | CPU à fond (whisper + release manquant) ou mauvaise sortie | `--release`, choisir sortie dans `pavucontrol` → Lecture |

## 9. Fedora / Arch (équivalences)

Fedora :

```bash
sudo dnf install -y gcc-c++ cmake pkgconf-pkg-config git curl wget \
  openssl-devel libxcb-devel libxkbcommon-devel mesa-libGL-devel \
  libXi-devel alsa-lib-devel systemd-devel \
  v4l-utils python3-pip pipewire pavucontrol
```

Arch :

```bash
sudo pacman -S --needed base-devel cmake pkgconf git curl wget \
  openssl libxcb libxkbcommon mesa libxi \
  alsa-lib systemd v4l-utils python-pip pipewire pavucontrol
```

Ensuite : rustup + Piper + modèles (§ 3–5 identiques).

## 10. Checklist première machine

- [ ] `v4l2-ctl --list-devices` → webcam vue
- [ ] `pactl list short sources/sinks` → micro + HP vus
- [ ] `python3 -m piper --help` → OK
- [ ] `aplay` test Piper (§ 5) → voix audible
- [ ] `cargo run --release` → fenêtre Aki + `[face.track] visage détecté`
- [ ] `Camille, bonjour` → `[voice] wake détecté` + réponse parlée
