use anyhow::{Context, Result};

use crate::conversion::{Asset, Importer, Joint, Scene};

use super::internal::{AngleBone, P3m, PositionBone};

pub struct P3mImporter {}

impl Importer for P3mImporter {
    fn import(&self, asset: &Asset, scene: &mut Scene) -> Result<()> {
        let p3m = P3m::from_bytes(&asset.bytes)
            .context("Failed to deserialize the bytes of the .p3m asset")?;

        scene.skeleton = convert_joints(&p3m.position_bones, &p3m.angle_bones);

        Ok(())
    }

    fn extensions(&self) -> &[&str] {
        &["p3m"]
    }
}

fn convert_joints(position_bones: &[PositionBone], angle_bones: &[AngleBone]) -> Vec<Joint> {
    let mut joints: Vec<_> = angle_bones.iter().map(|_| Joint::new()).collect();

    // Apply translation to the joints.
    for p_bone in position_bones {
        for &child in &p_bone.children {
            joints[child as usize].translation = p_bone.position.into();
        }
    }

    // Update joint children by squashing position and angle bones.
    for (joint, a_bone) in joints.iter_mut().zip(angle_bones) {
        let children = a_bone
            .children
            .iter()
            .flat_map(|&x| &position_bones[x as usize].children);
        for &child in children {
            joint.children.push(child as usize);
        }
    }

    // Set the parents of the joints.
    let mut joint_parents: Vec<_> = joints.iter().map(|_| None).collect();
    for (index, joint) in joints.iter().enumerate() {
        for &child in &joint.children {
            joint_parents[child] = Some(index);
        }
    }
    for (child, parent) in joint_parents.into_iter().enumerate() {
        joints[child].parent = parent;
    }

    joints
}
