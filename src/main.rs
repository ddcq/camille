#![allow(clippy::too_many_arguments)]
use fyrox::{
    core::{
        algebra::{UnitQuaternion, Vector3},
        color::Color,
        pool::Handle,
        reflect::prelude::*,
        visitor::prelude::*,
    },
    engine::executor::Executor,
    event::{ElementState, Event, MouseScrollDelta, WindowEvent},
    graph::SceneGraph,
    keyboard::KeyCode,
    plugin::{error::GameResult, Plugin, PluginContext, PluginRegistrationContext},
    resource::model::{Model, ModelResource, ModelResourceExtension},
    scene::{
        base::BaseBuilder,
        camera::CameraBuilder,
        graph::Graph,
        light::{directional::DirectionalLightBuilder, point::PointLightBuilder, BaseLightBuilder},
        node::Node,
        transform::Transform,
        EnvironmentLightingSource, Scene,
    },
};

mod expressions;
mod face_track;
mod jointtest;
mod mic;
mod tts;
mod voice;

const HEAD_BONE_NAME: &str = "J_Bip_C_Head";
const TRACK_SMOOTH: f32 = 0.35;

#[derive(Default, Debug, Visit, Reflect)]
#[reflect(non_cloneable)]
struct ProtoPlugin {
    model: Option<ModelResource>,
    spawned: bool,
    scene: Handle<Scene>,
    head: Option<Handle<Node>>,
    camera: Option<Handle<Node>>,
    time: f32,
    sm_yaw: f32,
    sm_pitch: f32,
    demo_mode: bool,
    // Contrôle caméra orbital.
    cam_yaw: f32,
    cam_pitch: f32,
    cam_dist: f32,
    cam_target_y: f32,
    keys: [bool; keystate::BINDS],
    #[reflect(hidden)]
    #[visit(skip)]
    expressions: expressions::State,
    #[reflect(hidden)]
    #[visit(skip)]
    joint_test: jointtest::State,
    #[reflect(hidden)]
    #[visit(skip)]
    last_speech_text: String,
}

mod keystate {
    pub const YAW_LEFT: usize = 0;
    pub const YAW_RIGHT: usize = 1;
    pub const PITCH_UP: usize = 2;
    pub const PITCH_DOWN: usize = 3;
    pub const DIST_IN: usize = 4;
    pub const DIST_OUT: usize = 5;
    pub const TARGET_UP: usize = 6;
    pub const TARGET_DOWN: usize = 7;
    pub const BINDS: usize = 8;
}

impl ProtoPlugin {
    fn setup_scene(graph: &mut Graph) {
        // Lumière : point fort pour éclairer le personnage.
        let light = PointLightBuilder::new(
            BaseLightBuilder::new(
                BaseBuilder::new()
                    .with_name("MainLight")
                    .with_local_transform(
                        Transform::identity().set_position(Vector3::new(0.0, 3.0, 2.0)).to_owned(),
                    ),
            )
            .with_intensity(6.0),
        )
        .with_radius(15.0)
        .build_node();
        let light_handle = graph.add_node(light);

        // Lumière directionnelle en complément.
        let dir_light = DirectionalLightBuilder::new(
            BaseLightBuilder::new(
                BaseBuilder::new()
                    .with_name("DirLight")
                    .with_local_transform(Transform::identity()
                        .set_position(Vector3::new(0.0, 0.0, 0.0))
                        .set_rotation(UnitQuaternion::from_euler_angles(0.5, 0.5, 0.0))
                        .to_owned()),
            )
            .with_intensity(2.0)
            .with_color(Color::opaque(255, 255, 255)),
        )
        .build_node();
        let dir_light = graph.add_node(dir_light);
        let _ = (dir_light, light_handle);

        // Caméra face au personnage.
        let camera = CameraBuilder::new(
            BaseBuilder::new()
                .with_name("Camera")
                .with_local_transform(
                    Transform::identity().set_position(Vector3::new(0.0, 1.4, 3.0)).to_owned(),
                ),
        )
        .build_node();
        let camera_handle = graph.add_node(camera);
        let _ = (camera_handle, light_handle);
    }
}

impl Plugin for ProtoPlugin {
    fn register(&self, _ctx: PluginRegistrationContext) -> GameResult {
        Ok(())
    }

    fn init(&mut self, _scene_path: Option<&str>, ctx: PluginContext) -> GameResult {
        let model: ModelResource =
            ctx.resource_manager.request::<Model>("assets/Aki.fbx");
        self.model = Some(model);
        face_track::start();
        if let Err(e) = mic::start() {
            println!("[socle] micro: {e}");
        }
        tts::start();
        voice::start();
        Ok(())
    }

    fn update(&mut self, ctx: &mut PluginContext) -> GameResult {
        if !self.spawned {
            if let Some(model) = &self.model {
                if model.is_ok() {
                    let mut scene = Scene::new();
                    scene
                        .rendering_options
                        .get_value_mut_and_mark_modified()
                        .environment_lighting_source = EnvironmentLightingSource::AmbientColor;
                    scene
                        .rendering_options
                        .get_value_mut_and_mark_modified()
                        .ambient_lighting_color = Color::opaque(120, 120, 120);
                    let root = model.instantiate(&mut scene);
                    scene.graph[root].set_scale_xyz(0.0066, 0.0066, 0.0066);
                    scene.graph[root].set_position(Vector3::new(0.0, 0.33, 0.0));
                    scene.graph[root].set_rotation(UnitQuaternion::from_euler_angles(
                        0.0,
                        std::f32::consts::PI,
                        0.0,
                    ));
                    Self::setup_scene(&mut scene.graph);

                    self.camera = scene
                        .graph
                        .linear_iter()
                        .position(|n| n.name().contains("Camera"))
                        .map(|i| scene.graph.handle_from_index(i as u32));
                    self.cam_yaw = 0.0;
                    self.cam_pitch = 0.1;
                    self.cam_dist = 0.35;
                    self.cam_target_y = 1.3;
                    for (i, node) in scene.graph.linear_iter().enumerate() {
                        if node.name() == HEAD_BONE_NAME {
                            self.head = Some(scene.graph.handle_from_index(i as u32));
                            break;
                        }
                    }
                    self.expressions.discover(&mut scene.graph);
                    self.joint_test.discover(&mut scene.graph, "J_Bip_C_Spine");
                    self.scene = ctx.scenes.add(scene);
                    self.spawned = true;
                }
            }
        } else {
            self.time += ctx.dt;
            // Visage → rotation du bone (lerp).
            let st = face_track::capture();
            let target_yaw = st.yaw;
            let target_pitch = st.pitch;
            if !self.demo_mode {
                let k = TRACK_SMOOTH;
                self.sm_yaw += (target_yaw - self.sm_yaw) * k;
                self.sm_pitch += (target_pitch - self.sm_pitch) * k;
            } else {
                // Sine démo.
                self.sm_yaw = (self.time * 1.2).sin() * 0.6;
                self.sm_pitch = (self.time * 1.7).sin() * 0.3;
            }
            if let Some(head) = self.head {
                // nalgebra: from_euler_angles(roulis_X, tangage_Y, lacet_Z).
                // Tête : haut/bas (nod) autour de X (axe épaules),
                // gauche/droite autour de Y (vertical).
                let quat =
                    UnitQuaternion::from_euler_angles(self.sm_pitch, self.sm_yaw, 0.0);
                let scene = ctx.scenes.try_get_mut(self.scene).unwrap();
                scene.graph[head].set_rotation(quat);
            }
            // Contrôle caméra orbital.
            let scene = ctx.scenes.try_get_mut(self.scene).unwrap();
            if let Some(cam) = self.camera {
                let rot_speed = 1.2 * ctx.dt;
                if self.keys[keystate::YAW_LEFT] {
                    self.cam_yaw -= rot_speed;
                }
                if self.keys[keystate::YAW_RIGHT] {
                    self.cam_yaw += rot_speed;
                }
                if self.keys[keystate::PITCH_UP] {
                    self.cam_pitch += rot_speed;
                }
                if self.keys[keystate::PITCH_DOWN] {
                    self.cam_pitch -= rot_speed;
                }
                if self.keys[keystate::DIST_IN] {
                    self.cam_dist -= 1.5 * ctx.dt;
                }
                if self.keys[keystate::DIST_OUT] {
                    self.cam_dist += 1.5 * ctx.dt;
                }
                if self.keys[keystate::TARGET_UP] {
                    self.cam_target_y += 0.5 * ctx.dt;
                }
                if self.keys[keystate::TARGET_DOWN] {
                    self.cam_target_y -= 0.5 * ctx.dt;
                }
                self.cam_dist = self.cam_dist.clamp(0.1, 30.0);
                self.cam_pitch = self.cam_pitch.clamp(-1.5, 1.5);
                self.cam_target_y = self.cam_target_y.clamp(-1.0, 2.5);

                let target = Vector3::new(0.0, self.cam_target_y, 0.0);
                let pos = target
                    + Vector3::new(
                        self.cam_dist * self.cam_pitch.cos() * self.cam_yaw.sin(),
                        self.cam_dist * self.cam_pitch.sin(),
                        self.cam_dist * self.cam_pitch.cos() * self.cam_yaw.cos(),
                    );
                let node = scene.graph.try_get_mut(cam).unwrap();
                node.local_transform_mut()
                    .set_position(pos)
                    .set_rotation(UnitQuaternion::face_towards(
                        &(target - pos),
                        &Vector3::new(0.0, 1.0, 0.0),
                    ));
            }
            self.expressions.update(ctx.dt);
            if let Some((text, dur, speaking)) = tts::state() {
                if speaking {
                    if self.last_speech_text != text {
                        self.last_speech_text = text.clone();
                        self.expressions.start_speech(&text, dur);
                    }
                }
            }
            self.expressions.apply(&mut scene.graph);
        }
        Ok(())
    }

    fn on_os_event(&mut self, event: &Event<()>, mut context: PluginContext) -> GameResult {
        use keystate::*;
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::KeyboardInput { event, .. } => {
                    let pressed = matches!(event.state, ElementState::Pressed);
                    let code = match event.physical_key {
                        fyrox::keyboard::PhysicalKey::Code(code) => code,
                        _ => return Ok(()),
                    };
                    let idx = match code {
                        KeyCode::ArrowLeft | KeyCode::KeyA => Some(YAW_LEFT),
                        KeyCode::ArrowRight | KeyCode::KeyD => Some(YAW_RIGHT),
                        KeyCode::ArrowUp | KeyCode::KeyW => Some(PITCH_UP),
                        KeyCode::ArrowDown | KeyCode::KeyS => Some(PITCH_DOWN),
                        KeyCode::KeyE => Some(DIST_IN),
                        KeyCode::KeyQ => Some(DIST_OUT),
                        KeyCode::KeyR => Some(TARGET_UP),
                        KeyCode::KeyF => Some(TARGET_DOWN),
                        _ => None,
                    };
                    if let Some(i) = idx {
                        self.keys[i] = pressed;
                    }
                    if pressed && matches!(code, KeyCode::Tab) {
                        self.demo_mode = !self.demo_mode;
                        println!(
                            "[proto] mode {}",
                            if self.demo_mode { "sinus (démo)" } else { "visage webcam" }
                        );
                    }
                    if pressed {
                        match code {
                            KeyCode::Digit1 => {
                                self.expressions.set_emotion(Some(expressions::Emotion::Joy))
                            }
                            KeyCode::Digit2 => self
                                .expressions
                                .set_emotion(Some(expressions::Emotion::Angry)),
                            KeyCode::Digit3 => self
                                .expressions
                                .set_emotion(Some(expressions::Emotion::Sorrow)),
                            KeyCode::Digit4 => self
                                .expressions
                                .set_emotion(Some(expressions::Emotion::Surprised)),
                            KeyCode::Digit5 => {
                                self.expressions.set_emotion(Some(expressions::Emotion::Fun))
                            }
                            KeyCode::Digit0 => self.expressions.set_emotion(None),
                            // Panneau de validation articulaire : épaule droite.
                            KeyCode::KeyU => self.joint_step(&mut context, jointtest::TestAxis::X, 1.0),
                            KeyCode::KeyI => self.joint_step(&mut context, jointtest::TestAxis::X, -1.0),
                            KeyCode::KeyO => self.joint_step(&mut context, jointtest::TestAxis::Y, 1.0),
                            KeyCode::KeyP => self.joint_step(&mut context, jointtest::TestAxis::Y, -1.0),
                            KeyCode::KeyT => self.joint_step(&mut context, jointtest::TestAxis::Z, 1.0),
                            KeyCode::KeyY => self.joint_step(&mut context, jointtest::TestAxis::Z, -1.0),
                            KeyCode::KeyM => self.joint_refresh(&mut context),
                            KeyCode::Space => self
                                .expressions
                                .start_speech("Bonjour, je m'appelle Camille et je suis ravie de vous rencontrer", 3.0),
                            _ => {}
                        }
                    }
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    if let MouseScrollDelta::LineDelta(_, dy) = delta {
                        self.cam_dist -= dy * 0.1;
                        self.cam_dist = self.cam_dist.clamp(0.1, 30.0);
                    }
                }
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }
}

impl ProtoPlugin {
    fn joint_step(&mut self, context: &mut PluginContext<'_, '_>, axis: jointtest::TestAxis, dir: f32) {
        if let Ok(scene) = context.scenes.try_get_mut(self.scene) {
            self.joint_test.step(&mut scene.graph, axis, dir);
        }
    }

    fn joint_refresh(&mut self, context: &mut PluginContext<'_, '_>) {
        if let Ok(scene) = context.scenes.try_get_mut(self.scene) {
            self.joint_test.reset(&mut scene.graph);
        }
    }
}

fn main() {
    let mut executor = Executor::new(Some(fyrox::event_loop::EventLoop::new().unwrap()));
    executor.add_plugin(ProtoPlugin::default());
    executor.run();
}