use fyrox::{
    core::pool::Handle,
    graph::SceneGraph,
    scene::{graph::Graph, mesh::Mesh, node::Node},
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Emotion {
    Joy,
    Angry,
    Sorrow,
    Surprised,
    Fun,
}

impl Emotion {
    fn morphs(&self) -> &'static [(&'static str, f32)] {
        match self {
            Emotion::Joy => &[
                ("Fcl_ALL_Joy", 60.0),
                ("Fcl_BRW_Joy", 45.0),
                ("Fcl_EYE_Joy_L", 30.0),
                ("Fcl_EYE_Joy_R", 30.0),
                ("Fcl_MTH_Joy", 50.0),
            ],
            Emotion::Angry => &[
                ("Fcl_ALL_Angry", 60.0),
                ("Fcl_BRW_Angry", 55.0),
                ("Fcl_EYE_Angry", 40.0),
                ("Fcl_MTH_Angry", 45.0),
            ],
            Emotion::Sorrow => &[
                ("Fcl_ALL_Sorrow", 60.0),
                ("Fcl_BRW_Sorrow", 40.0),
                ("Fcl_EYE_Sorrow", 35.0),
                ("Fcl_MTH_Sorrow", 45.0),
            ],
            Emotion::Surprised => &[
                ("Fcl_ALL_Surprised", 70.0),
                ("Fcl_BRW_Surprised", 60.0),
                ("Fcl_EYE_Surprised", 55.0),
                ("Fcl_MTH_Surprised", 50.0),
            ],
            Emotion::Fun => &[
                ("Fcl_ALL_Fun", 55.0),
                ("Fcl_BRW_Fun", 40.0),
                ("Fcl_EYE_Fun", 30.0),
                ("Fcl_MTH_Fun", 35.0),
            ],
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Viseme {
    A,
    I,
    U,
    E,
    O,
}

impl Viseme {
    fn morph_name(&self) -> &'static str {
        match self {
            Viseme::A => "Fcl_MTH_A",
            Viseme::I => "Fcl_MTH_I",
            Viseme::U => "Fcl_MTH_U",
            Viseme::E => "Fcl_MTH_E",
            Viseme::O => "Fcl_MTH_O",
        }
    }

    fn from_char(c: char) -> Option<Viseme> {
        match c.to_ascii_lowercase() {
            'a' | 'à' | 'â' => Some(Viseme::A),
            'i' | 'î' | 'ï' | 'y' => Some(Viseme::I),
            'u' | 'û' | 'ü' => Some(Viseme::U),
            'e' | 'é' | 'è' | 'ê' | 'ë' => Some(Viseme::E),
            'o' | 'ô' => Some(Viseme::O),
            _ => None,
        }
    }
}

#[derive(Debug)]
struct MorphEntry {
    name: String,
    mesh: Handle<Node>,
    index: usize,
}

const MICRO_SMILE: f32 = 12.0;
const SPEECH_WEIGHT: f32 = 70.0;
const SMOOTH_K: f32 = 8.0;
const EMOTION_HOLD: f32 = 0.4;
const EMOTION_RELEASE: f32 = 0.2;

#[derive(Debug, Default)]
pub struct State {
    morphs: Vec<MorphEntry>,
    weights: Vec<f32>,
    // Emotion.
    emotion: Option<Emotion>,
    emotion_t: f32,
    // Parole (visèmes).
    visemes: Vec<(f32, Viseme)>,
    speech_t: f32,
    speech_active: bool,
}

impl State {
    pub fn discover(&mut self, graph: &mut Graph) {
        self.morphs.clear();
        let mut seen = std::collections::HashSet::new();
        for (i, node) in graph.linear_iter().enumerate() {
            if let Some(mesh) = node.cast::<Mesh>() {
                for (idx, bs) in mesh.blend_shapes().iter().enumerate() {
                    let name = bs.name.as_str();
                    if name.starts_with("Fcl_")
                        && !name.ends_with('S')
                        && seen.insert(name.to_string())
                    {
                        self.morphs.push(MorphEntry {
                            name: name.to_string(),
                            mesh: graph.handle_from_index(i as u32),
                            index: idx,
                        });
                    }
                }
            }
        }
        self.weights = vec![0.0; self.morphs.len()];
    }

    pub fn start_speech(&mut self, text: &str, dur: f32) {
        let len = text.chars().count().max(1) as f32;
        let mut v = Vec::new();
        for (i, c) in text.chars().enumerate() {
            if let Some(vis) = Viseme::from_char(c) {
                let t = (i as f32 / len) * dur;
                v.push((t, vis));
            }
        }
        v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        self.visemes = v;
        self.speech_t = 0.0;
        self.speech_active = true;
    }

    pub fn set_emotion(&mut self, emotion: Option<Emotion>) {
        self.emotion = emotion;
        self.emotion_t = 0.0;
    }

    pub fn update(&mut self, dt: f32) {
        if self.morphs.is_empty() {
            return;
        }
        self.update_emotion(dt);
        self.update_speech(dt);

        let mut targets = std::collections::BTreeMap::new();
        targets.insert("Fcl_MTH_Fun".to_string(), MICRO_SMILE);
        if let Some(vis) = self.active_viseme() {
            targets.insert(vis.morph_name().to_string(), SPEECH_WEIGHT);
        }
        if let Some(em) = self.emotion.as_ref() {
            let k = self.emotion_scale();
            for (name, w) in em.morphs() {
                let target = targets.entry(name.to_string()).or_insert(0.0);
                *target += w * k;
            }
        }

        for (i, entry) in self.morphs.iter().enumerate() {
            let target = targets.get(&entry.name).copied().unwrap_or(0.0).min(100.0);
            let w = &mut self.weights[i];
            *w += (target - *w) * (1.0 - (-SMOOTH_K * dt).exp());
            if *w < 0.5 {
                *w = 0.0;
            }
        }
    }

    pub fn apply(&self, graph: &mut Graph) {
        for (i, entry) in self.morphs.iter().enumerate() {
            let w = self.weights[i];
            if w == 0.0 {
                continue;
            }
            if let Ok(node) = graph.try_get_mut(entry.mesh) {
                if let Some(mesh) = node.cast_mut::<Mesh>() {
                    if let Some(bs) = mesh.blend_shapes_mut().get_mut(entry.index) {
                        bs.weight = w;
                    }
                }
            }
        }
    }

    fn update_emotion(&mut self, dt: f32) {
        if self.emotion.is_some() {
            self.emotion_t += dt;
            if self.emotion_t > EMOTION_HOLD + EMOTION_RELEASE {
                self.emotion = None;
            }
        }
    }

    fn emotion_scale(&self) -> f32 {
        if self.emotion_t <= EMOTION_HOLD {
            1.0
        } else {
            (1.0 - (self.emotion_t - EMOTION_HOLD) / EMOTION_RELEASE).clamp(0.0, 1.0)
        }
    }

    fn update_speech(&mut self, dt: f32) {
        if !self.speech_active {
            return;
        }
        self.speech_t += dt;
        let end = match self.visemes.last() {
            Some((t, _)) => *t,
            None => {
                self.speech_active = false;
                return;
            }
        };
        if self.speech_t > end + 0.25 {
            self.speech_active = false;
        }
    }

    fn active_viseme(&self) -> Option<Viseme> {
        if !self.speech_active {
            return None;
        }
        self.visemes
            .iter()
            .filter(|(t, _)| *t <= self.speech_t)
            .map(|(_, v)| *v)
            .next_back()
    }
}