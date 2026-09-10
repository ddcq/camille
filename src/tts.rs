use std::io::Write;
use std::process::{Command, Stdio};
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

// Voix française Piper (siwis medium, 22050 Hz). Synthèse via `python3 -m piper`.
const PIPER_MODEL: &str = "models/piper/fr_FR-siwis-medium.onnx";
const PIPER_RATE: u32 = 22050;

pub fn start() {
    let (tx, rx): (Sender<String>, Receiver<String>) = channel();
    let state = Arc::new(Mutex::new(State::default()));
    std::thread::spawn({
        let state = state.clone();
        move || worker(rx, state)
    });
    let _ = TTS.set(Tts { cmd: tx, state });
}

/// Sortie brute PCM 16-bit LE mono via le binaire Piper (module python).
fn synth_piper(text: &str) -> Result<Vec<f32>, String> {
    let mut child = Command::new("python3")
        .args(["-m", "piper", "--model", PIPER_MODEL, "--output_raw"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("spawn piper: {e}"))?;
    child
        .stdin
        .as_mut()
        .ok_or("piper: pas de stdin")?
        .write_all(text.as_bytes())
        .map_err(|e| format!("piper stdin: {e}"))?;
    let out = child
        .wait_with_output()
        .map_err(|e| format!("piper wait: {e}"))?;
    if !out.status.success() {
        return Err(format!("piper: exit {:?}", out.status.code()));
    }
    let bytes = out.stdout;
    if bytes.len() < 2 {
        return Err("piper: sortie vide".to_string());
    }
    let mut samples = Vec::with_capacity(bytes.len() / 2);
    for chunk in bytes.chunks_exact(2) {
        let v = i16::from_le_bytes([chunk[0], chunk[1]]);
        samples.push(v as f32 / 32768.0);
    }
    Ok(samples)
}

fn worker(rx: Receiver<String>, state: Arc<Mutex<State>>) {
    let sink = match rodio::DeviceSinkBuilder::open_default_sink() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[tts] sortie audio: {e}");
            return;
        }
    };
    let player = rodio::Player::connect_new(&sink.mixer());

    while let Ok(text) = rx.recv() {
        let samples = match synth_piper(&text) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[tts] synthèse piper: {e}");
                continue;
            }
        };
        let dur_secs = samples.len() as f32 / PIPER_RATE as f32;
        {
            let mut s = state.lock().unwrap();
            s.text = text.clone();
            s.dur_secs = dur_secs;
            s.speaking = true;
        }
        println!("[tts] lecture ({dur_secs:.1}s): «{text}»");
        let nz_ch = std::num::NonZeroU16::new(1).unwrap();
        let nz_rate = std::num::NonZeroU32::new(PIPER_RATE).unwrap();
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
