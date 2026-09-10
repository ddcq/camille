use anyhow::Context;
use mediapipe::{FaceLandmarker, Image, ModelSource, Size};
use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{CameraIndex, RequestedFormat, RequestedFormatType};
use nokhwa::Camera;
use std::sync::{Arc, Mutex, OnceLock};

const EYE_LEFT_OUTER: usize = 33;
const EYE_RIGHT_OUTER: usize = 263;
const GAIN: f32 = 1.2;

pub struct FaceState {
    pub yaw: f32,
    pub pitch: f32,
    pub has_face: bool,
}

pub type FaceHandle = Arc<Mutex<FaceState>>;

fn handle() -> &'static FaceHandle {
    static HANDLE: OnceLock<FaceHandle> = OnceLock::new();
    HANDLE.get_or_init(|| Arc::new(Mutex::new(FaceState {
        yaw: 0.0,
        pitch: 0.0,
        has_face: false,
    })))
}

pub fn start() {
    let state = handle().clone();
    std::thread::spawn(move || {
        if let Err(e) = run(state) {
            eprintln!("[face.track] ERREUR: {e:#}");
        }
    });
}

fn run(state: FaceHandle) -> anyhow::Result<()> {
    let requested =
        RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);
    let mut camera = Camera::new(CameraIndex::Index(0), requested).context("ouverture caméra")?;
    camera.open_stream().context("open_stream")?;
    println!("[face.track] caméra ouverte");

    let mut landmarker = FaceLandmarker::builder(ModelSource::path("face_landmarker.task"))
        .build()
        .context("build FaceLandmarker")?;
    println!("[face.track] landmarker prêt");

    let mut was_face = false;
    loop {
        let frame = camera.frame().context("capture frame")?;
        let decoded = frame.decode_image::<RgbFormat>().context("decode RGB")?;
        let (w, h) = (decoded.width(), decoded.height());
        let image = Image::from_rgb(Size { width: w, height: h }, decoded.as_raw())
            .context("Image::from_rgb")?;
        let result = landmarker.detect(&image).context("detect")?;

        if let Some(face_landmarks) = result.landmarks.first() {
            let l = &face_landmarks[EYE_LEFT_OUTER];
            let r = &face_landmarks[EYE_RIGHT_OUTER];
            let cx = (l.point.x() + r.point.x()) / 2.0;
            let cy = (l.point.y() + r.point.y()) / 2.0;
            // Position dans l'image → yaw/pitch direct.
            let yaw = (0.5 - cx) * GAIN;
            let pitch = (0.5 - cy) * GAIN;

            let mut st = state.lock().unwrap();
            st.yaw = yaw;
            st.pitch = pitch;
            st.has_face = true;
            if !was_face {
                println!("[face.track] visage détecté (cx={cx:.2} cy={cy:.2})");
            }
            was_face = true;
        } else {
            let mut st = state.lock().unwrap();
            st.has_face = false;
            if was_face {
                println!("[face.track] visage perdu");
            }
            was_face = false;
        }
    }
}

pub fn capture() -> FaceState {
    let st = handle().lock().unwrap();
    FaceState {
        yaw: st.yaw,
        pitch: st.pitch,
        has_face: st.has_face,
    }
}