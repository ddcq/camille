use fyrox::{
    core::pool::Handle,
    graph::SceneGraph,
    scene::{graph::Graph, node::Node},
    core::algebra::{UnitQuaternion, Vector3},
};

// Panneau de validation articulaire : pose l'os par rotations locales
// incrémentales, une touche = un pas autour d'un axe LOCAL.
// But : vérifier l'orientation réelle de chaque articulation de Camille
// (axes du FBX = qu'on sm arrive pas à déduire du modèle).

pub const STEP_DEG: f32 = 15.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TestAxis {
    X,
    Y,
    Z,
}

#[derive(Debug)]
pub struct State {
    pub name: String,
    pub handle: Option<Handle<Node>>,
    pub bind: UnitQuaternion<f32>,
    pub last: UnitQuaternion<f32>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            name: String::new(),
            handle: None,
            bind: UnitQuaternion::identity(),
            last: UnitQuaternion::identity(),
        }
    }
}

impl State {
    pub fn discover(&mut self, graph: &Graph, joint_name: &str) {
        self.name = joint_name.to_string();
        self.handle = None;
        let mut chain: Vec<(String, Vector3<f32>)> = Vec::new();
        for (i, n) in graph.linear_iter().enumerate() {
            if n.name() == joint_name {
                let h = graph.handle_from_index(i as u32);
                self.handle = Some(h);
                self.bind = **graph[h].local_transform().rotation();
                self.last = self.bind;
                // Remonte la chaîne parentale jusqu'à la racine, affiche noms + positions monde.
                let mut cur = Some(h);
                while let Some(c) = cur {
                    if c.is_none() {
                        break;
                    }
                    let node = &graph[c];
                    chain.push((node.name().to_string(), node.global_position()));
                    cur = node.parent().into();
                }
                println!(
                    "[jointtest] articulations de test: {joint_name} (bind: {:?})",
                    self.last.euler_angles()
                );
                // Axes monde du bind (pour comparer le repère de repos avec le BVH).
                let world = graph.global_rotation(h);
                for (label, v) in [
                    ("X", Vector3::x()),
                    ("Y", Vector3::y()),
                    ("Z", Vector3::z()),
                ] {
                    let a = world * v;
                    println!(
                        "[jointtest]   {joint_name} world-{label}: ({:+.3}, {:+.3}, {:+.3})",
                        a.x, a.y, a.z
                    );
                }
                // Affiche chaîne du plus profond au plus haut (leaf → root).
                for (name, pos) in chain.iter().rev() {
                    println!(
                        "[jointtest]   {:28} pos: ({:.3}, {:.3}, {:.3})",
                        name, pos.x, pos.y, pos.z
                    );
                }
                return;
            }
        }
        println!("[jointtest] articulation introuvable: {joint_name}");
    }

    pub fn reset(&mut self, graph: &mut Graph) {
        self.last = self.bind;
        if let Some(h) = self.handle {
            if let Ok(node) = graph.try_get_mut(h) {
                node.local_transform_mut().set_rotation(self.bind);
            }
        }
        println!("[jointtest] reset -> bind");
    }

    /// Applique un pas de rotation autour de l'axe local, pré-multiplié
    /// (`rot * cur` : rotation dans le repère local de l'os), dévient clavier.
    pub fn step(&mut self, graph: &mut Graph, axis: TestAxis, dir: f32) {
        let Some(h) = self.handle else {
            println!("[jointtest] pas d'articulation prêt (aucun handle)");
            return;
        };
        let angle = STEP_DEG * dir;
        let v = match axis {
            TestAxis::X => &Vector3::x_axis(),
            TestAxis::Y => &Vector3::y_axis(),
            TestAxis::Z => &Vector3::z_axis(),
        };
        let rot = UnitQuaternion::from_axis_angle(v, angle.to_radians());
        self.last = rot * self.last;
        if let Ok(node) = graph.try_get_mut(h) {
            node.local_transform_mut().set_rotation(self.last);
        }
        println!(
            "[jointtest] {} {}: {:+}° (quat: {:?}, euler: {:?})",
            self.name,
            match axis {
                TestAxis::X => "X",
                TestAxis::Y => "Y",
                TestAxis::Z => "Z",
            },
            angle,
            self.last.coords.as_slice(),
            self.last.euler_angles()
        );
    }
}