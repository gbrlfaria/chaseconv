use glam::{Mat4, Vec2, Vec3A};

/// A 3D scene that contains meshes, animations, and a skeleton. Left-handed Y-up. It's the intermediary format between conversions and provides some geometric facilitites.
pub struct Scene {
    meshes: Vec<Mesh>,
    skeleton: Vec<Joint>,
    animations: Vec<Animation>,
}

pub struct Mesh {
    vertices: Vec<Vertex>,
    indices: Vec<usize>,
}

/// no rotation or scale
pub struct Joint {
    /// Relative to the parent
    translation: Vec3A,
    children: Vec<usize>,
}

pub struct Animation {
    frames: Vec<Keyframe>,
}

pub struct Vertex {
    position: Vec3A,
    normal: Vec3A,
    uv: Vec2,
    /// 100% influence
    joint: usize,
}

pub struct Keyframe {
    time: u64,
    root_translation: Vec3A,
    bone_transforms: Vec<Mat4>,
}
