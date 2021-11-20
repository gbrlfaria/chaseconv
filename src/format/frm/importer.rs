use anyhow::Context;
use glam::{Mat4, Vec3A};

use crate::conversion::{Animation, Asset, Importer, Keyframe, Scene};

use super::internal::Frm;

pub struct FrmImporter {}

impl Importer for FrmImporter {
    fn import(&self, asset: &Asset, scene: &mut Scene) -> anyhow::Result<()> {
        let frm = Frm::from_bytes(&asset.bytes)
            .context("Failed to deserialize the bytes of the FRM asset")?;

        let animation = Animation {
            name: asset.name().to_string(),
            frames: convert_frames(&frm),
        };
        scene.animations.push(animation);

        Ok(())
    }

    fn extensions(&self) -> &[&str] {
        &["frm"]
    }
}

fn convert_frames(frm: &Frm) -> Vec<Keyframe> {
    let mut prev_root_trans = Vec3A::new(0., 0., 0.);

    frm.frames
        .iter()
        .map(|frame| {
            let keyframe = Keyframe {
                duration: 1000. / 55.,
                root_translation: Vec3A::new(
                    prev_root_trans.x + frame.plus_x,
                    frame.pos_y,
                    prev_root_trans.z + frame.pos_z,
                ),
                joint_transforms: frame
                    .bones
                    .iter()
                    .map(|transform| Mat4::from_cols_array_2d(transform).transpose())
                    .collect(),
            };

            prev_root_trans = keyframe.root_translation;

            keyframe
        })
        .collect()
}
