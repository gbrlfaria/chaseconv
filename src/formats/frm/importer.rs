use anyhow::Context;
use glam::{Mat4, Vec3A};

use crate::{
    asset::Asset,
    conversion::Importer,
    scene::{Animation, Keyframe, Scene},
};

use super::internal::Frm;

#[derive(Default)]
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
    let mut current_time = 0.;

    frm.frames
        .iter()
        .map(|frame| {
            let keyframe = Keyframe {
                // The Z coordinate is set to zero because it is unused in the game and usually
                // contains unreasonable values.
                translation: Vec3A::new(prev_root_trans.x + frame.plus_x, frame.pos_y, frame.pos_z),
                transforms: frame.bones.iter().map(Mat4::from_cols_array_2d).collect(),
            };

            // The frame rate of the animation is always 55 FPS.
            current_time += 1. / 55.;
            prev_root_trans = keyframe.translation;

            keyframe
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use crate::formats::frm::internal::{Frame, FrmVersion};

    use super::*;

    #[test]
    fn frames() {
        let frm = Frm {
            version: FrmVersion::V1_1,
            frames: vec![
                Frame {
                    option: 0,
                    plus_x: 1.,
                    pos_y: 1.,
                    pos_z: 1.,
                    bones: vec![[[1.; 4]; 4], [[2.; 4]; 4]],
                },
                Frame {
                    option: 0,
                    plus_x: 1.,
                    pos_y: 1.,
                    pos_z: 1.,
                    bones: vec![[[3.; 4]; 4], [[4.; 4]; 4]],
                },
            ],
        };

        let actual = convert_frames(&frm);
        let expected = vec![
            Keyframe {
                translation: Vec3A::new(1., 1., 1.),
                transforms: vec![
                    Mat4::from_cols_array(&[1.; 16]),
                    Mat4::from_cols_array(&[2.; 16]),
                ],
            },
            Keyframe {
                translation: Vec3A::new(2., 1., 1.),
                transforms: vec![
                    Mat4::from_cols_array(&[3.; 16]),
                    Mat4::from_cols_array(&[4.; 16]),
                ],
            },
        ];

        assert_eq!(expected, actual);
    }
}
