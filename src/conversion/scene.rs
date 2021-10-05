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

/// Represents the geometry of a mesh.
pub struct Mesh {
    /// The list of vertices (vertex buffer) of the geometry.
    pub vertices: Vec<Vertex>,
    /// The list of indexes (index buffer) of the geometry, which determines the faces of the mesh.
    pub indexes: Vec<usize>,
}

/// Represents a joint of the [`Scene`] skeleton. It only supports translation.
pub struct Joint {
    /// The translation of the joint, relative to its parent.
    translation: Vec3A,
    /// The list of children of the joint. Each element of the list represents
    /// the index of a children in the [`Scene`] skeleton.
    /// A joint should have a maximum of 10 children.
    children: Vec<usize>,
}

/// Represents a keyframe animation sequence.
pub struct Animation {
    frames: Vec<Keyframe>,
}

/// Represents a skinned vertex of a mesh.
pub struct Vertex {
    /// The position of the vertex, relative to the origin.
    position: Vec3A,
    /// The normal vector of the vertex.
    normal: Vec3A,
    /// The UV-mapping texture coordinates of the vertex.
    uv: Vec2,
    /// The index of the single influencing joint in the [`Scene`] skeleton.
    /// The joint exerts 100% influence over the vertex.
    joint: usize,
}

/// Represents a single keyframe of a animation sequence.
pub struct Keyframe {
    /// The duration, in milliseconds, of the frame.
    duration: u64,
    /// The translation of applied to the whole skeleton.
    root_translation: Vec3A,
    /// The list transform matrices for each joint at the current frame.
    /// Each matrix in the list should correspond to the joint with same
    /// index in the [`Scene`] skeleton.
    joint_transforms: Vec<Mat4>,
}
