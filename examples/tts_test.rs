// Test TTS isolé (sans fenêtre Fyrox ni webcam) :
//   cargo run --example tts_test -- "Bonjour Camille"
// Valide chargement voix + synthèse + lecture sur une machine fraîche (ex. toolbox Fedora).
use piper_rs::Piper;
use std::path::Path;

fn main() {
    let text = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "Bonjour Camille.".to_string());
    let mut piper = Piper::new(
        Path::new("models/piper/fr_FR-siwis-medium.onnx"),
        Path::new("models/piper/fr_FR-siwis-medium.onnx.json"),
    )
    .expect("chargement voix piper");
    let (samples, rate) = piper
        .create(&text, false, None, None, None, None)
        .expect("synthèse piper");
    println!(
        "[tts-test] «{text}» → {} échantillons @ {rate} Hz ({:.1}s)",
        samples.len(),
        samples.len() as f32 / rate as f32
    );
    let sink = rodio::DeviceSinkBuilder::open_default_sink().expect("sortie audio");
    let player = rodio::Player::connect_new(sink.mixer());
    let nz_ch = std::num::NonZeroU16::new(1).unwrap();
    let nz_rate = std::num::NonZeroU32::new(rate).unwrap();
    player.append(rodio::buffer::SamplesBuffer::new(nz_ch, nz_rate, samples));
    player.sleep_until_end();
    println!("[tts-test] lecture terminée");
}
