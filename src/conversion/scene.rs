use glam::{Mat4, Vec2, Vec3A};

/// Represents a 3D scene comprised of an skeleton, meshes, and animations.
/// It's the intermediary format between conversions and provides some operations.
///
/// The geometry should use the left-handed Y-up coordinate system.
pub struct Scene {
    pub meshes: Vec<Mesh>,
    pub skeleton: Vec<Joint>,
    pub animations: Vec<Animation>,
}

impl Scene {
    /// Returns the translation of the joint with the given index, relative to the origin of
    /// the scene.
    pub fn joint_world_translation(&self, index: usize) -> Vec3A {
        let mut joint = &self.skeleton[index];
        let mut translation = joint.translation;
        while let Some(parent) = joint.parent {
            joint = &self.skeleton[parent];
            translation += joint.translation;
        }

        translation
    }
}

/// Represents the geometry of a mesh.
#[derive(Debug, PartialEq)]
pub struct Mesh {
    /// The name of the mesh.
    pub name: String,
    /// The list of vertices (vertex buffer) of the geometry.
    pub vertices: Vec<Vertex>,
    /// The list of indexes (index buffer) of the geometry, which determines the faces of the mesh.
    pub indexes: Vec<usize>,
}

/// Represents a joint of the [`Scene`] skeleton. It only supports translation.
#[derive(Debug, PartialEq)]
pub struct Joint {
    /// The translation of the joint, relative to its parent.
    pub translation: Vec3A,
    /// The index of the parent of the joint. The index refers to the [`Scene`] skeleton.
    pub parent: Option<usize>,
    /// The indexes of the children of the joint. The indexes refer to the [`Scene`] skeleton.
    /// A joint should have a maximum of 10 children.
    pub children: Vec<usize>,
}

impl Joint {
    pub fn new() -> Self {
        Self {
            translation: Vec3A::new(0., 0., 0.),
            parent: None,
            children: Vec::new(),
        }
    }
}

/// Represents a keyframe animation sequence.
pub struct Animation {
    pub name: String,
    pub frames: Vec<Keyframe>,
}

/// Represents a skinned vertex of a mesh.
#[derive(Debug, PartialEq)]
pub struct Vertex {
    /// The position of the vertex, relative to the origin.
    pub position: Vec3A,
    /// The normal vector of the vertex.
    pub normal: Vec3A,
    /// The UV-mapping texture coordinates of the vertex.
    pub uv: Vec2,
    /// The index of the single influencing joint in the [`Scene`] skeleton.
    /// The joint exerts 100% influence over the vertex.
    pub joint: usize,
}

/// Represents a single keyframe of a animation sequence.
pub struct Keyframe {
    /// The duration, in milliseconds, of the frame.
    pub duration: f64,
    /// The translation of applied to the whole skeleton.
    pub root_translation: Vec3A,
    /// The list transform matrices for each joint at the current frame.
    /// Each matrix in the list should correspond to the joint with same
    /// index in the [`Scene`] skeleton.
    pub joint_transforms: Vec<Mat4>,
}
