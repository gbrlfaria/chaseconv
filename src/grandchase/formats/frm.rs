use std::io::{Cursor, Read, Result, Seek, SeekFrom, Write};

use byteorder::{ReadBytesExt, WriteBytesExt, LE};

const NEW_VERSION_HEADER: &str = "Frm Ver 1.1\0";

/// Represents an FRM file. The FRM format stores keyframe animation data.
pub struct Frm {
    /// The version of the FRM.
    pub version: FrmVersion,

    /// The frames of the animation over time. The frames are supposed to be played at 55 FPS.
    pub frames: Vec<Frame>,

    /// The translation of the whole skeleton along the Z axis over time. It is only present in
    /// FRM v1.1. There is one translation value for each frame of the animation.
    pub pos_z: Vec<f32>,
}

impl Frm {
    pub fn new(version: FrmVersion) -> Self {
        Self {
            version,
            frames: Vec::new(),
            pos_z: Vec::new(),
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut reader = Cursor::new(bytes);

        let mut header = [0 as u8; NEW_VERSION_HEADER.len()];
        reader.read_exact(&mut header)?;

        let frm = if header != NEW_VERSION_HEADER.as_bytes() {
            let mut frm = Self::new(FrmVersion::V1_0);

            let (num_frames, num_bones) = (reader.read_u8()?, reader.read_u8()?);
            for _ in 0..num_frames {
                frm.frames
                    .push(Frame::from_reader(&mut reader, num_bones as u16)?);
            }

            frm
        } else {
            let mut frm = Self::new(FrmVersion::V1_1);

            reader.seek(SeekFrom::Start(0))?;

            let (num_frames, num_bones) = (reader.read_u16::<LE>()?, reader.read_u16::<LE>()?);
            for _ in 0..num_frames {
                frm.frames.push(Frame::from_reader(&mut reader, num_bones)?);
            }
            for _ in 0..num_frames {
                frm.pos_z.push(reader.read_f32::<LE>()?);
            }

            frm
        };

        Ok(frm)
    }

    pub fn into_bytes(&self) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();

        match self.version {
            FrmVersion::V1_0 => {
                bytes.write(NEW_VERSION_HEADER.as_bytes())?;
                bytes.write_u16::<LE>(self.frames.len() as u16)?;
                bytes.write_u16::<LE>(self.num_bones() as u16)?;

                for frame in &self.frames {
                    frame.into_bytes(&mut bytes)?;
                }
            }
            FrmVersion::V1_1 => {
                bytes.write_u8(self.frames.len() as u8)?;
                bytes.write_u8(self.num_bones() as u8)?;

                for frame in &self.frames {
                    frame.into_bytes(&mut bytes)?;
                }
                for &z in &self.pos_z {
                    bytes.write_f32::<LE>(z)?;
                }
            }
        }

        Ok(bytes)
    }

    pub fn num_bones(&self) -> usize {
        if self.frames.is_empty() {
            0
        } else {
            self.frames[0].bones.len()
        }
    }
}

/// Represents an animation keyframe.
pub struct Frame {
    /// Unused field. It's defaulted to `0`.
    option: u8,

    /// The translation of the entire skeleton over the x axis for the current frame.
    plus_x: f32,

    /// The translation of the entire skeleton over the y axis for the current frame.
    pos_y: f32,

    /// The rotation matrices of all bones for the current frame.
    bones: Vec<[[f32; 4]; 4]>,
}

impl Frame {
    pub fn new() -> Self {
        Self {
            option: 0,
            plus_x: 0.,
            pos_y: 0.,
            bones: Vec::new(),
        }
    }

    pub fn from_reader(reader: &mut Cursor<&[u8]>, num_bones: u16) -> Result<Self> {
        let mut frame = Self::new();

        frame.option = reader.read_u8()?;
        frame.plus_x = reader.read_f32::<LE>()?;
        frame.pos_y = reader.read_f32::<LE>()?;

        for _ in 0..num_bones {
            let mut bone = [[0. as f32; 4]; 4];
            for row in bone.iter_mut() {
                reader.read_f32_into::<LE>(row)?;
            }
            frame.bones.push(bone);
        }

        Ok(frame)
    }

    pub fn into_bytes(&self, bytes: &mut Vec<u8>) -> Result<()> {
        bytes.write_u8(self.option)?;
        bytes.write_f32::<LE>(self.plus_x)?;
        bytes.write_f32::<LE>(self.pos_y)?;

        for bone_matrix in &self.bones {
            for row in bone_matrix {
                for &element in row {
                    bytes.write_f32::<LE>(element)?;
                }
            }
        }

        Ok(())
    }
}

/// Determines the binary format of the FRM file.
pub enum FrmVersion {
    V1_0,
    V1_1,
}
