# Camille sur Fedora ostree — installer, lancer, développer

Cible unique : **Fedora Atomic (Silverblue / Kinoite / ublue), base ostree immuable**.
Principe : **rien n'est installé sur le système hôte** — tout le dev vit dans un
container **toolbox** (préinstallé, partage `/dev`, audio PipeWire et affichage Wayland/X11
avec l'hôte). Aucun `rpm-ostree install`, aucun reboot.

## 1. Ce que fait Camille (rappel utile pour le dev)

- Fenêtre 3D **Fyrox 1.0** : avatar Aki (`assets/aki.glb`), caméra orbitale.
- Thread webcam (**nokhwa** V4L2) → **MediaPipe FaceLandmarker** (`face_landmarker.task`) → yaw/pitch tête (`src/face_track.rs`).
- Thread micro (**cpal** via `mic.rs`, 16 kHz mono, ring buffer 8 s) → **whisper-cpp** (`models/ggml-tiny.bin`, `src/voice.rs`) → wake word `camille` → commande → réponse.
- TTS **Piper** via `python3 -m piper` (`models/piper/fr_FR-siwis-medium.onnx`) → lecture **rodio** (`src/tts.rs`).

Logs au démarrage : `[face.track]`, `[voice]`, `[tts]`.

## 2. Hôte : vérifications (sans rien installer)

```bash
rpm-ostree status               # confirme deployment ostree
ls -l /dev/video*               # webcam vue par le noyau ?
toolbox --version               # préinstallé sur Silverblue/Kinoite
```

- Webcam : ouvrir **Paramètres → Confidentialité → Caméra** ou tester avec Cheese (flatpak).
- Micro/HP : **Paramètres → Son** → entrée/sortie par défaut.
- Groupes (persiste dans `/etc`, autorisé sur ostree) :

```bash
sudo usermod -aG video $USER
# reconnexion (ou reboot) pour prise en compte
```

Optionnel côté hôte (flatpaks, pas de layering) :

```bash
flatpak install -y flathub org.pulseaudio.pavucontrol   # routage audio
flatpak install -y flathub org.gnome.Cheese              # test webcam
```

> Ne pas `rpm-ostree install` les `-devel` : tout ça va dans toolbox (§ 3).

## 3. Toolbox `camille` + paquets dev

Le home est partagé hôte ↔ toolbox : cloner dans `~/projects` rend le code
éditable depuis l'hôte et compilable depuis toolbox.

```bash
toolbox create camille
toolbox enter camille

# --- à l'intérieur de toolbox ---
sudo dnf install -y \
  gcc-c++ cmake pkgconf-pkg-config git curl wget \
  openssl-devel \
  libxcb-devel libxkbcommon-devel libX11-devel libXi-devel \
  mesa-libGL-devel mesa-libEGL-devel \
  alsa-lib-devel alsa-utils systemd-devel \
  v4l-utils \
  python3 python3-pip \
  pipewire-utils
```

| Paquet | Pourquoi |
|---|---|
| `gcc-c++`, `cmake` | `whisper-cpp-plus` compile whisper.cpp au premier `cargo build` |
| `libxcb-devel`, `libxkbcommon-devel`, `libXi-devel`, `mesa-*-devel` | fenêtre + OpenGL Fyrox/winit |
| `alsa-lib-devel`, `systemd-devel` | `cpal`/`rodio` : micro + HP |
| `v4l-utils` | `v4l2-ctl` (test webcam depuis toolbox) |
| `alsa-utils` | `aplay` (test Piper) |
| `python3-pip` | module `piper-tts` en venv (§ 5) |

Vérifier périphériques **depuis toolbox** (partage `/dev` + PipeWire) :

```bash
v4l2-ctl --list-devices        # doit lister /dev/video0
pactl list short sources       # micro visible
pactl list short sinks         # sortie audio visible
```

## 4. Rust (dans toolbox, via rustup)

```bash
# --- dans toolbox ---
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
rustc --version   # viser stable ≥ 1.80
```

> rustup vit dans `$HOME`, donc partagé/persistant. Ne pas utiliser le paquet
> `rust` du dépôt (souvent en retard et source de conflits `cargo`).

## 5. Piper TTS (venv dans toolbox)

`src/tts.rs` lance `python3 -m piper --model models/piper/fr_FR-siwis-medium.onnx --output_raw`.
Créer un venv et **toujours lancer Camille avec ce venv activé** :

```bash
# --- dans toolbox ---
python3 -m venv ~/.venvs/camille
~/.venvs/camille/bin/pip install piper-tts
source ~/.venvs/camille/bin/activate
python3 -m piper --help   # doit afficher l'aide
```

## 6. Cloner + modèles

```bash
# --- dans toolbox, venv activé ---
mkdir -p ~/projects && cd ~/projects
git clone https://github.com/ddcq/camille.git
cd camille
mkdir -p models/piper kokoro/models kokoro/voices
```

Les modèles lourds (~270 Mo) ne sont **pas** dans git (voir `.gitignore`) :

| Fichier | Destination |
|---|---|
| `face_landmarker.task` (3,6 Mo, déjà dans git) | racine |
| `ggml-tiny.bin` (whisper, ~75 Mo) | `models/` |
| `ggml-silero-v6.2.0.bin` (VAD, optionnel) | `models/` |
| `fr_FR-siwis-medium.onnx{,.json}` (Piper) | `models/piper/` |
| `model_quantized.onnx` + `ff_siwis.bin` (Kokoro, optionnel) | `kokoro/` |

```bash
# 1. FaceLandmarker — déjà dans git, sinon :
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

Test Piper de bout en bout (dans toolbox, venv activé) :

```bash
echo "Bonjour Camille" | python3 -m piper \
  --model models/piper/fr_FR-siwis-medium.onnx --output_raw \
  | aplay -r 22050 -f S16_LE -t raw -c 1
# doit parler. Sinon : modèle absent, venv oublié, ou mauvaise sortie (pavucontrol).
```

## 7. Compiler + lancer (dans toolbox)

La fenêtre Fyrox s'affiche sur l'hôte (toolbox partage Wayland/X11).

```bash
# --- dans toolbox, depuis ~/projects/camille, venv activé ---
cargo run --release
```

- Premier build : 5–15 min (whisper.cpp + Fyrox). Suivants incrémentaux.
- Itérations 3D rapides : `cargo run` (profil dev, `opt-level = 1` dans `Cargo.toml`).
- Qualité avant commit :

```bash
cargo fmt --check
cargo clippy -- -D warnings
```

Sortie attendue :

```
[face.track] caméra ouverte
[face.track] landmarker prêt
[face.track] visage détecté (cx=0.48 cy=0.51)
[voice] entendu (rms=0.0123): «...»
```

Dire **`Camille, bonjour`** → `[voice] wake détecté` + `[tts] lecture` parlée.
Contrôles : molette = zoom, clavier = orbite caméra.

Sans webcam/micro (dev 3D pur) : `[face.track] ERREUR: ouverture caméra` ou
`aucun micro par défaut` — la fenêtre reste utilisable, tête immobile.

### One-liner depuis l'hôte (sans `toolbox enter`)

```bash
toolbox run -c camille bash -lc \
  'source ~/.venvs/camille/bin/activate && cd ~/projects/camille && cargo run --release'
```

## 8. Continuer à programmer — où toucher

```
src/main.rs        Plugin Fyrox, scène, caméra, boucle. Point d'entrée des features.
src/face_track.rs  Thread V4L2 + MediaPipe → FaceState{yaw,pitch}. Constante GAIN=1.2.
src/expressions.rs Morphs visage (Joy/Angry/Sorrow/Surprised/Fun) + visèmes.
src/jointtest.rs   Calibration articulations.
src/mic.rs         Capture cpal 16 kHz, ring buffer. API: start/window/pos/since.
src/voice.rs       Wake word + whisper + brain(). Constantes WIN_SECS/TICK_MS.
src/tts.rs         File TTS → synth_piper() → rodio. PIPER_MODEL/PIPER_RATE.
```

Workflow ostree : **éditer sur l'hôte** (VS Code flatpak, GNOME Text — fichiers
dans `~/projects` partagés), **compiler/lancer dans toolbox**.

Recettes :

**Commande vocale** → `src/voice.rs`, `brain()` :

```rust
} else if c.contains("souris") {
    "Bien sûr, je souris.".to_string()
}
```

`cargo run`, dire `Camille, souris`.

**Émotion** → `src/expressions.rs`, `Emotion::morphs()` (noms `Fcl_*` = morph
targets du `.glb` Aki — vérifier dans le modèle avant d'inventer).

**Head-tracking** → `src/face_track.rs` (`GAIN`, yeux `33`/`263`), lissage
`TRACK_SMOOTH` dans `main.rs`.

**Voix Piper** → autre `.onnx` fr, MAJ `PIPER_MODEL` + `PIPER_RATE`
(22050 Hz pour siwis-medium, cf `.onnx.json`).

Boucle conseillée (dans toolbox) :

```bash
cargo check          # rapide, à chaque edit
cargo clippy
cargo run            # test 3D seul
cargo run --release  # test voix temps réel (whisper trop lent en debug)
```

## 9. Dépannage (spécifique ostree + Camille)

| Symptôme | Cause probable | Fix |
|---|---|---|
| `dnf install` refuse (`/usr` read-only) | commande lancée sur l'hôte | tout le § 3–7 se fait **dans** `toolbox enter camille` |
| `ouverture caméra` / `open_stream` | webcam absente ou prise (navigateur), groupe `video` | § 2 : `ls /dev/video*`, Cheese, `usermod -aG video`, fermer l'autre app |
| Fenêtre ne s'affiche pas (Wayland) | backend winit | `WINIT_UNIX_BACKEND=x11 toolbox run -c camille ...` (XWayland), ou session GNOME Xorg |
| `aucun micro par défaut` | entrée désactivée | Paramètres → Son, ou `pavucontrol` (flatpak) → Entrée |
| `spawn piper` / `No module named piper` | venv non activé | `source ~/.venvs/camille/bin/activate` avant `cargo run` |
| `chargement whisper` | `models/ggml-tiny.bin` absent | re-`wget` § 6, `ls -lh models/` (~75 Mo) |
| `cmake not found` / erreur C++ | paquets toolbox oubliés | § 3 puis `cargo clean -p whisper-cpp-plus` |
| Whisper très lent | build debug | `--release` obligatoire pour tests voix |
| `face_landmarker.task` introuvable | mauvais cwd | lancer depuis `~/projects/camille` (chemin relatif dans `face_track.rs`) |
| Son saccadé | CPU (debug) ou mauvaise sortie | `--release`, `pavucontrol` → Lecture |
| Rendu 3D lent / `LIBGL` software | pas d'accélération dans toolbox | `glxinfo -B` (dans toolbox) doit citer le GPU ; sinon mettre à jour pilotes hôte via update système, pas de layering Mesa |

Recréer toolbox en cas de casse (code et venv dans `$HOME`, donc intacts) :

```bash
toolbox rm camille && toolbox create camille
# puis refaire § 3 (dnf) — venv/rustup/projets conservés
```

## 10. Checklist première machine

- [ ] `rpm-ostree status` → ostree confirmé, `ls /dev/video*` → webcam
- [ ] `toolbox create camille` → OK, § 3 installé
- [ ] `v4l2-ctl --list-devices` (toolbox) → webcam
- [ ] `pactl list short sources/sinks` (toolbox) → micro + HP
- [ ] `python3 -m piper --help` (venv) → OK, test `aplay` → voix audible
- [ ] `cargo run --release` (toolbox) → fenêtre Aki + `[face.track] visage détecté`
- [ ] `Camille, bonjour` → `[voice] wake détecté` + réponse parlée
