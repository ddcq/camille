# Camille sur Fedora ostree — installer, lancer, développer

Cible unique : **Fedora Atomic (Silverblue / Kinoite / ublue), base ostree immuable**.
Principe : **rien n'est installé sur le système hôte** — tout le dev vit dans un
container **toolbox** (préinstallé, partage `/dev`, audio PipeWire et affichage Wayland/X11
avec l'hôte). Aucun `rpm-ostree install`, aucun reboot.

## 1. Ce que fait Camille (rappel utile pour le dev)

- Fenêtre 3D **Fyrox 1.0** : avatar Aki (`assets/aki.glb`), caméra orbitale.
- Thread webcam (**nokhwa** V4L2) → **MediaPipe FaceLandmarker** (`face_landmarker.task`) → yaw/pitch tête (`src/face_track.rs`).
- Thread micro (**cpal** via `mic.rs`, 16 kHz mono, ring buffer 8 s) → **whisper-cpp** (`models/ggml-tiny.bin`, `src/voice.rs`) → wake word `camille` → commande → réponse.
- TTS **Piper** in-process (`piper-rs`: espeak-ng + onnxruntime, `models/piper/fr_FR-siwis-medium.onnx`) → lecture **rodio** (`src/tts.rs`). Zéro dépendance python.

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
  pipewire-utils
```

| Paquet | Pourquoi |
|---|---|
| `gcc-c++`, `cmake` | `whisper-cpp-plus` (whisper.cpp) + `espeak-rs-sys` (espeak-ng) compilés au premier `cargo build` |
| `libxcb-devel`, `libxkbcommon-devel`, `libXi-devel`, `mesa-*-devel` | fenêtre + OpenGL Fyrox/winit |
| `alsa-lib-devel`, `systemd-devel` | `cpal`/`rodio` : micro + HP |
| `v4l-utils` | `v4l2-ctl` (test webcam depuis toolbox) |
| `alsa-utils` | `aplay` (test audio) |

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

## 5. Piper TTS — rien à installer

Synthèse 100 % Rust via la crate `piper-rs` (espeak-ng + onnxruntime compilés
avec le projet au premier `cargo build`). Pas de venv, pas de module python,
pas de binaire externe. Seuls les fichiers voix comptent (voir § 6) :

## 6. Cloner + modèles

```bash
# --- dans toolbox, venv activé ---
mkdir -p ~/projects && cd ~/projects
git clone https://github.com/ddcq/camille.git
cd camille
mkdir -p models/piper
```

Les modèles lourds (~140 Mo utiles) ne sont **pas** dans git (voir `.gitignore`) :

| Fichier | Destination |
|---|---|
| `face_landmarker.task` (3,6 Mo, déjà dans git) | racine |
| `ggml-tiny.bin` (whisper STT, ~77 Mo) — obligatoire pour les commandes vocales | `models/` |
| `fr_FR-siwis-medium.onnx{,.json}` (voix Piper, ~63 Mo) — obligatoire pour la parole | `models/piper/` |

```bash
# 1. FaceLandmarker — déjà dans git, sinon :
wget -O face_landmarker.task \
  https://storage.googleapis.com/mediapipe-models/face_landmarker/face_landmarker/float16/1/face_landmarker.task

# 2. Whisper tiny (fr compris) — chargé par src/voice.rs :
wget -O models/ggml-tiny.bin \
  https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin

# 3. Voix Piper fr_FR siwis medium — chargée par src/tts.rs (PIPER_MODEL) :
wget -O models/piper/fr_FR-siwis-medium.onnx \
  https://huggingface.co/rhasspy/piper-voices/resolve/main/fr/fr_FR/siwis/medium/fr_FR-siwis-medium.onnx
wget -O models/piper/fr_FR-siwis-medium.onnx.json \
  https://huggingface.co/rhasspy/piper-voices/resolve/main/fr/fr_FR/siwis/medium/fr_FR-siwis-medium.onnx.json
```

Vérifier :

```bash
ls -lh models/ggml-tiny.bin models/piper/
# ggml-tiny.bin ~77 Mo, fr_FR-siwis-medium.onnx ~63 Mo + .onnx.json ~5 Ko
```

> Obsolètes (non référencés par le code depuis le passage à Piper, supprimables
> pour ~140 Mo) : `kokoro/` (ancien TTS), `models/piper/piper/` + `models/piper.tgz`
> (ancien binaire natif — le code appelle `python3 -m piper`), `models/ggml-silero-v6.2.0.bin`
> (VAD non chargé). Détail dans README § Modèles.

Test voix de bout en bout, sans lancer tout Camille (dans toolbox) :

```bash
cargo run --example tts_test -- "Bonjour Camille"
# attendus : «N échantillons @ 22050 Hz», puis voix audible.
# Sinon : modèle absent (§ 6), ou mauvaise sortie (pavucontrol).
```

## 7. Compiler + lancer (dans toolbox)

La fenêtre Fyrox s'affiche sur l'hôte (toolbox partage Wayland/X11).

```bash
# --- dans toolbox, depuis ~/projects/camille ---
cargo run --release
```

- Premier build : 10–25 min (whisper.cpp + espeak-ng + onnxruntime + Fyrox ; onnxruntime prébuild téléchargé, réseau requis). Suivants incrémentaux.
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
toolbox run -c camille bash -lc 'cd ~/projects/camille && cargo run --release'
```

## 8. Continuer à programmer — où toucher

```
src/main.rs        Plugin Fyrox, scène, caméra, boucle. Point d'entrée des features.
src/face_track.rs  Thread V4L2 + MediaPipe → FaceState{yaw,pitch}. Constante GAIN=1.2.
src/expressions.rs Morphs visage (Joy/Angry/Sorrow/Surprised/Fun) + visèmes.
src/jointtest.rs   Calibration articulations.
src/mic.rs         Capture cpal 16 kHz, ring buffer. API: start/window/pos/since.
src/voice.rs       Wake word + whisper + brain(). Constantes WIN_SECS/TICK_MS.
src/tts.rs         File TTS → piper-rs in-process → rodio. Constantes PIPER_MODEL/PIPER_CONFIG.
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
| `[tts] chargement voix piper` | `.onnx` / `.onnx.json` absents ou corrompus | re-`wget` § 6, `ls -lh models/piper/` (~63 Mo + ~5 Ko) |
| `chargement whisper` | `models/ggml-tiny.bin` absent | re-`wget` § 6, `ls -lh models/` (~77 Mo) |
| `cmake not found` / erreur C++ | paquets toolbox oubliés | § 3 puis `cargo clean -p whisper-cpp-plus -p espeak-rs-sys` |
| Whisper très lent | build debug | `--release` obligatoire pour tests voix |
| `face_landmarker.task` introuvable | mauvais cwd | lancer depuis `~/projects/camille` (chemin relatif dans `face_track.rs`) |
| Son saccadé | CPU (debug) ou mauvaise sortie | `--release`, `pavucontrol` → Lecture |
| Rendu 3D lent / `LIBGL` software | pas d'accélération dans toolbox | `glxinfo -B` (dans toolbox) doit citer le GPU ; sinon mettre à jour pilotes hôte via update système, pas de layering Mesa |

Recréer toolbox en cas de casse (code et rustup dans `$HOME`, donc intacts) :

```bash
toolbox rm camille && toolbox create camille
# puis refaire § 3 (dnf) — rustup/projets conservés
```

## 10. Checklist première machine

- [ ] `rpm-ostree status` → ostree confirmé, `ls /dev/video*` → webcam
- [ ] `toolbox create camille` → OK, § 3 installé
- [ ] `v4l2-ctl --list-devices` (toolbox) → webcam
- [ ] `pactl list short sources/sinks` (toolbox) → micro + HP
- [ ] `cargo run --example tts_test -- "Bonjour Camille"` (toolbox) → échantillons + voix audible
- [ ] `cargo run --release` (toolbox) → fenêtre Aki + `[face.track] visage détecté`
- [ ] `Camille, bonjour` → `[voice] wake détecté` + réponse parlée
