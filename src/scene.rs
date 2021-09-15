use glam::{Mat4, Vec2, Vec3A};

/// A 3D scene comprised of an skeleton, meshes, and animations.
/// It's the intermediary format between conversions and provides some geometric facilitites.
///
/// The geometry should use the left-handed Y-up coordinate system.
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
    /// The translation of the joint, relative to its parent.
    translation: Vec3A,
    /// The list of children of the joint. Each element of the list represents
    /// the index of a children in the [`Scene`] skeleton.
    /// A joint should have at most 10 children.
    children: Vec<usize>,
}

pub struct Animation {
    frames: Vec<Keyframe>,
}

pub struct Vertex {
    position: Vec3A,
    normal: Vec3A,
    uv: Vec2,
    /// The index of the single influencing joint in the [`Scene`] skeleton.
    /// The joint exerts 100% influence over the vertex.
    joint: usize,
}

pub struct Keyframe {
    time: u64,
    root_translation: Vec3A,
    bone_transforms: Vec<Mat4>,
}
