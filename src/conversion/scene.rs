use glam::{Mat4, Vec2, Vec3A};

/// Represents a 3D scene comprised of skeleton, meshes, and animations.
/// It's the intermediary format between conversions and provides some operations.
///
/// It should use the left-handed Y-up coordinate system.
#[derive(Debug, Default, Clone)]
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

    pub fn merge(mut self, mut other: Scene) -> Self {
        if self.skeleton.is_empty() {
            self.skeleton = other.skeleton;
        }
        self.meshes.append(&mut other.meshes);
        self.animations.append(&mut other.animations);

        self
    }
}

/// Represents the geometry of a mesh.
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Mesh {
    /// The name of the mesh.
    pub name: String,
    /// The list of vertices (vertex buffer) of the geometry.
    pub vertices: Vec<Vertex>,
    /// The list of indices (index buffer) of the geometry, which determines the faces of the mesh.
    pub indices: Vec<usize>,
}

/// Represents a joint of the [`Scene`] skeleton. It only supports translation.
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Joint {
    /// The translation of the joint, relative to its parent.
    pub translation: Vec3A,
    /// The index of the parent of the joint. The index refers to the [`Scene`] skeleton.
    pub parent: Option<usize>,
    /// The indexes of the children of the joint. The indexes refer to the [`Scene`] skeleton.
    /// A joint should have a maximum of 10 children.
    pub children: Vec<usize>,
}

/// Represents a keyframe animation sequence. It should be sampled at 55 FPS.
#[derive(Debug, Clone)]
pub struct Animation {
    pub name: String,
    pub frames: Vec<Keyframe>,
}

impl Animation {
    pub fn joints(&self) -> Vec<Vec<&Mat4>> {
        let mut result = Vec::new();
        for frame in &self.frames {
            for (index, rotation) in frame.transforms.iter().enumerate() {
                if index >= result.len() {
                    result.push(Vec::new());
                }
                result[index].push(rotation);
            }
        }
        result
    }

    pub fn sampling_rate(&self) -> i32 {
        55
    }
}

/// Represents a skinned vertex of a mesh.
#[derive(Debug, PartialEq, Clone)]
pub struct Vertex {
    /// The position of the vertex, relative to the origin.
    pub position: Vec3A,
    /// The normal vector of the vertex.
    pub normal: Vec3A,
    /// The UV-mapping texture coordinates of the vertex.
    pub uv: Vec2,
    /// The index of the single influencing joint in the [`Scene`] skeleton.
    /// The joint exerts 100% influence over the vertex.
    pub joint: Option<usize>,
}

/// Represents a single keyframe of a animation sequence.
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Keyframe {
    /// The translation of applied to the whole skeleton.
    pub translation: Vec3A,
    /// The list of matrices for each joint at the current frame.
    /// Each matrix in the list should correspond to the joint with same
    /// index in the [`Scene`] skeleton.
    pub transforms: Vec<Mat4>,
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn joint_world_translation() {
        let scene = Scene {
            meshes: Vec::new(),
            skeleton: vec![
                Joint {
                    translation: Vec3A::new(1., 1., 1.),
                    parent: None,
                    children: vec![1, 2],
                },
                Joint {
                    translation: Vec3A::new(2., 2., 2.),
                    parent: Some(0),
                    children: vec![3],
                },
                Joint {
                    translation: Vec3A::new(4., 4., 4.),
                    parent: Some(0),
                    children: Vec::new(),
                },
                Joint {
                    translation: Vec3A::new(0., 0., 0.),
                    parent: Some(1),
                    children: Vec::new(),
                },
            ],
            animations: Vec::new(),
        };

        let actual = scene.joint_world_translation(0);
        let expected = Vec3A::new(1., 1., 1.);
        assert_eq!(expected, actual);

        let actual = scene.joint_world_translation(1);
        let expected = Vec3A::new(3., 3., 3.);
        assert_eq!(expected, actual);

        let actual = scene.joint_world_translation(2);
        let expected = Vec3A::new(5., 5., 5.);
        assert_eq!(expected, actual);

        let actual = scene.joint_world_translation(3);
        let expected = Vec3A::new(3., 3., 3.);
        assert_eq!(expected, actual);
    }
}
