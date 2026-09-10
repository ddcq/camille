use std::sync::OnceLock;

use whisper_cpp_plus::{TranscriptionParams, WhisperContext};

use crate::tts;

const WAKE_WORD: &str = "camille";
const WIN_SECS: f32 = 2.50;
const SESSION_SECS: f32 = 2.50;
const TICK_MS: u64 = 250;

static CTX: OnceLock<Result<WhisperContext, String>> = OnceLock::new();

fn ctx() -> Option<&'static WhisperContext> {
    CTX.get_or_init(|| {
        WhisperContext::new("models/ggml-tiny.bin").map_err(|e| {
            let msg = format!("chargement whisper: {e}");
            eprintln!("[voice] {msg}");
            msg
        })
    })
    .as_ref()
    .ok()
}

fn wake_params() -> TranscriptionParams {
    TranscriptionParams::builder()
        .language("fr")
        .temperature(0.0)
        .build()
}

fn transcribe(audio: &[f32]) -> Option<String> {
    let c = ctx()?;
    c.transcribe_with_params(audio, wake_params())
        .ok()
        .map(|r| r.text.trim().to_string())
}

fn contains_wake(text: &str) -> bool {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .any(|w| w == WAKE_WORD)
}

fn command_from(text: &str) -> Option<String> {
    let mut seen_wake = false;
    let mut parts: Vec<&str> = Vec::new();
    let lower = text.to_lowercase();
    for tok in lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
    {
        if tok == WAKE_WORD {
            seen_wake = true;
            continue;
        }
        if seen_wake {
            parts.push(tok);
        }
    }
    if seen_wake {
        Some(parts.join(" "))
    } else {
        None
    }
}

fn brain(command: &str) -> String {
    let c = command.trim();
    if c.is_empty() {
        "Oui, je vous écoute.".to_string()
    } else if c.contains("bonjour") || c.contains("salut") {
        "Bonjour ! Ravi de vous voir.".to_string()
    } else {
        format!("Vous avez dit : {c}. J'en prends note.")
    }
}

pub fn start() {
    if CTX.get().is_some() {
        return;
    }
    // Initialise le contexte Whisper au démarrage (une seule fois).
    std::thread::spawn(|| {
        let _ = ctx();
    });

    std::thread::spawn(|| loop {
        crate::mic::sleep_ms(TICK_MS);
        if !crate::mic::is_started() {
            continue;
        }
        if tts::is_speaking() {
            continue;
        }

        let mut win = Vec::new();
        let n = crate::mic::window(WIN_SECS, &mut win);
        if n == 0 {
            continue;
        }
        let rms = (win.iter().map(|s| s * s).sum::<f32>() / win.len() as f32).sqrt();
        if rms < 0.008 {
            continue;
        }

        let text = match transcribe(&win) {
            Some(t) => t,
            None => continue,
        };
        // Debug écoute : montre ce que Camille entend à chaque fenêtre sonore.
        if !text.is_empty() {
            println!("[voice] entendu (rms={rms:.4}): «{text}»");
        }
        if !contains_wake(&text) {
            continue;
        }
        println!("[voice] wake détecté: «{text}»");

        // "Camille, <commande>" dans la même fenêtre ?
        let immediate = command_from(&text);
        let reply = match immediate {
            Some(c) if !c.trim().is_empty() => {
                println!("[voice] commande immédiate: «{c}»");
                brain(&c)
            }
            _ => {
                // Wake seul → session : on écoute la suite.
                println!("[voice] session d'écoute ({SESSION_SECS}s)...");
                let start_pos = match crate::mic::pos() {
                    Some(p) => p,
                    None => continue,
                };
                loop {
                    crate::mic::sleep_ms(TICK_MS);
                    if tts::is_speaking() {
                        break;
                    }
                    let mut buf = Vec::new();
                    let n2 = crate::mic::since(start_pos, &mut buf);
                    if n2 as f32 / 16000.0 >= SESSION_SECS {
                        break;
                    }
                }
                let mut buf = Vec::new();
                let _ = crate::mic::since(start_pos, &mut buf);
                let cmd = transcribe(&buf)
                    .and_then(|t| command_from(&t))
                    .unwrap_or_default();
                println!("[voice] commande entendue: «{cmd}»");
                brain(&cmd)
            }
        };
        tts::speak(&reply);
        println!("[voice] réponse: «{reply}»");
    });
}