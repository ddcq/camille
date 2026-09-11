use piper_rs::Piper;
use std::path::Path;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Clone)]
pub struct Tts {
    cmd: Sender<String>,
    state: Arc<Mutex<State>>,
}

#[derive(Default, Clone)]
pub struct State {
    pub text: String,
    pub dur_secs: f32,
    pub speaking: bool,
}

static TTS: OnceLock<Tts> = OnceLock::new();

// Voix française Piper (siwis medium). Synthèse in-process via piper-rs
// (espeak-ng + onnxruntime) — plus de dépendance python ni binaire externe.
const PIPER_MODEL: &str = "models/piper/fr_FR-siwis-medium.onnx";
const PIPER_CONFIG: &str = "models/piper/fr_FR-siwis-medium.onnx.json";

pub fn start() {
    let (tx, rx): (Sender<String>, Receiver<String>) = channel();
    let state = Arc::new(Mutex::new(State::default()));
    std::thread::spawn({
        let state = state.clone();
        move || worker(rx, state)
    });
    let _ = TTS.set(Tts { cmd: tx, state });
}

/// Synthèse in-process : texte → (échantillons f32 mono, sample rate).
/// La session est créée une fois par le worker, pas à chaque phrase
/// (l'ancien appel `python3 -m piper` rechargeait le modèle à chaque fois).
fn synth_piper(piper: &mut Piper, text: &str) -> Result<(Vec<f32>, u32), String> {
    piper
        .create(text, false, None, None, None, None)
        .map_err(|e| format!("piper-rs: {e}"))
}

fn worker(rx: Receiver<String>, state: Arc<Mutex<State>>) {
    let mut piper = match Piper::new(Path::new(PIPER_MODEL), Path::new(PIPER_CONFIG)) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[tts] chargement voix piper: {e}");
            return;
        }
    };
    println!("[tts] voix piper chargée ({PIPER_MODEL})");
    let sink = match rodio::DeviceSinkBuilder::open_default_sink() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[tts] sortie audio: {e}");
            return;
        }
    };
    let player = rodio::Player::connect_new(sink.mixer());

    while let Ok(text) = rx.recv() {
        let (samples, rate) = match synth_piper(&mut piper, &text) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[tts] synthèse piper: {e}");
                continue;
            }
        };
        let dur_secs = samples.len() as f32 / rate as f32;
        {
            let mut s = state.lock().unwrap();
            s.text = text.clone();
            s.dur_secs = dur_secs;
            s.speaking = true;
        }
        println!("[tts] lecture ({dur_secs:.1}s): «{text}»");
        let nz_ch = std::num::NonZeroU16::new(1).unwrap();
        let nz_rate = std::num::NonZeroU32::new(rate).unwrap();
        let source = rodio::buffer::SamplesBuffer::new(nz_ch, nz_rate, samples);
        player.append(source);
        player.sleep_until_end();
        {
            let mut s = state.lock().unwrap();
            s.speaking = false;
        }
    }
    drop(sink);
}

pub fn speak(text: &str) {
    if let Some(t) = TTS.get() {
        let _ = t.cmd.send(text.to_string());
    }
}

pub fn state() -> Option<(String, f32, bool)> {
    let t = TTS.get()?;
    let s = t.state.lock().unwrap();
    Some((s.text.clone(), s.dur_secs, s.speaking))
}

pub fn is_speaking() -> bool {
    state().map(|(_, _, s)| s).unwrap_or(false)
}
