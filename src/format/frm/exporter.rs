use anyhow::Result;
use glam::Vec3A;

use crate::{
    conversion::{Animation, Asset, Exporter, Scene},
    format::frm::internal::Frm,
};

use super::internal::{Frame, FrmVersion};

#[derive(Default)]
pub struct FrmExporter {}

impl Exporter for FrmExporter {
    fn export(&self, scene: &Scene) -> Result<Vec<Asset>> {
        let mut result = Vec::new();
        for animation in &scene.animations {
            let frm = Frm {
                version: FrmVersion::V1_1,
                frames: convert_frames(animation),
            };

            let name = if !animation.name.is_empty() {
                &animation.name
            } else {
                "animation"
            };
            let asset = Asset::new(frm.to_bytes()?, &format!("{}.frm", name));

            result.push(asset);
        }

        Ok(result)
    }
}

// The algorithm assumes the animation keyframes are already sampled at 55 FPS.
fn convert_frames(animation: &Animation) -> Vec<Frame> {
    let mut prev_root_trans = Vec3A::new(0., 0., 0.);
    animation
        .frames
        .iter()
        .map(|keyframe| {
            let frame = Frame {
                option: 0,
                plus_x: keyframe.translation.x - prev_root_trans.x,
                pos_y: keyframe.translation.y,
                pos_z: keyframe.translation.z - prev_root_trans.z,
                bones: keyframe
                    .transforms
                    .iter()
                    .map(|matrix| matrix.to_cols_array_2d())
                    .collect(),
            };
            prev_root_trans = keyframe.translation;

            frame
        })
        .collect()
}
