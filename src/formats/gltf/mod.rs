pub use {exporter::GltfExporter, importer::GltfImporter};

use glam::{Mat4, Vec4};

use crate::scene::Scene;

mod exporter;
mod importer;

fn transform(scene: &Scene) -> Scene {
    let mut scene = scene.clone();

    let mut matrix = Mat4::IDENTITY;
    matrix.z_axis = Vec4::new(0., 0., -1., 0.);

    for mesh in &mut scene.meshes {
        for vertex in &mut mesh.vertices {
            vertex.position = matrix.transform_point3a(vertex.position);
            vertex.normal = matrix.transform_point3a(vertex.normal);
        }

        for i in 0..mesh.indices.len() / 3 {
            mesh.indices.swap(i * 3 + 1, i * 3 + 2);
        }
    }

    for joint in &mut scene.skeleton {
        joint.translation = matrix.transform_point3a(joint.translation);
    }

    for animation in &mut scene.animations {
        for frame in &mut animation.frames {
            frame.translation.z *= -1.;
            for transform in &mut frame.transforms {
                *transform = matrix.mul_mat4(transform).mul_mat4(&matrix.inverse());
            }
        }
    }

    scene
}
