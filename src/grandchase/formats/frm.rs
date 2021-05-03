use std::io::{Cursor, Read, Result, Seek, SeekFrom, Write};

use byteorder::{ReadBytesExt, WriteBytesExt, LE};

const VERSION_HEADER: &str = "Frm Ver 1.1\0";

/// Represents an FRM file. The FRM format stores keyframe animation data from GrandChase.
#[derive(Debug, PartialEq)]
pub struct Frm {
    /// The version of the FRM.
    pub version: FrmVersion,

    /// The frames of the animation over time. The frames are supposed to be played at 55 FPS.
    pub frames: Vec<Frame>,

    /// The translation of the entire skeleton along the Z axis over time. It is only present in
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

        let mut header = [0; VERSION_HEADER.len()];
        reader.read_exact(&mut header)?;

        let frm = if header != VERSION_HEADER.as_bytes() {
            let mut frm = Self::new(FrmVersion::V1_0);

            reader.seek(SeekFrom::Start(0))?;

            let num_frames = reader.read_u8()?;
            let num_bones = reader.read_u8()?;
            for _ in 0..num_frames {
                frm.frames
                    .push(Frame::from_reader(&mut reader, num_bones as u16)?);
            }

            frm
        } else {
            let mut frm = Self::new(FrmVersion::V1_1);

            let num_frames = reader.read_u16::<LE>()?;
            let num_bones = reader.read_u16::<LE>()?;
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

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();

        match self.version {
            FrmVersion::V1_0 => {
                bytes.write_u8(self.frames.len() as u8)?;
                bytes.write_u8(self.num_bones() as u8)?;

                for frame in &self.frames {
                    frame.into_bytes(&mut bytes)?;
                }
            }
            FrmVersion::V1_1 => {
                bytes.write(VERSION_HEADER.as_bytes())?;
                bytes.write_u16::<LE>(self.frames.len() as u16)?;
                bytes.write_u16::<LE>(self.num_bones() as u16)?;

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
        match self.frames.first() {
            Some(frame) => frame.bones.len(),
            None => 0,
        }
    }
}

/// Represents an animation keyframe.
#[derive(Debug, PartialEq)]
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
            let mut bone = [[0.; 4]; 4];
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
#[derive(Debug, PartialEq, Eq)]
pub enum FrmVersion {
    V1_0,
    V1_1,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_v1_0() {
        let (expected, bytes) = data_v1_0();
        let actual = Frm::from_bytes(&bytes).unwrap();

        assert_eq!(expected, actual);
    }

    #[test]
    fn write_v1_0() {
        let (frm, expected) = data_v1_0();
        let actual = frm.to_bytes().unwrap();

        assert_eq!(expected, actual);
    }

    #[test]
    fn read_v1_1() {
        let (expected, bytes) = data_v1_1();
        let actual = Frm::from_bytes(&bytes).unwrap();

        assert_eq!(expected, actual);
    }

    #[test]
    fn write_v1_1() {
        let (frm, expected) = data_v1_1();
        let actual = frm.to_bytes().unwrap();

        assert_eq!(expected, actual);
    }

    fn data_v1_0() -> (Frm, &'static [u8]) {
        let frm = Frm {
            version: FrmVersion::V1_0,
            frames: vec![
                Frame {
                    option: 0,
                    plus_x: 1.,
                    pos_y: -1.,
                    bones: vec![[[0.; 4], [0.; 4], [0.; 4], [0.; 4]]],
                },
                Frame {
                    option: 0,
                    plus_x: -1.,
                    pos_y: 1.,
                    bones: vec![[[1.; 4], [1.; 4], [1.; 4], [1.; 4]]],
                },
            ],
            pos_z: Vec::new(),
        };

        const DATA: [u8; 148] = [
            0x02, 0x01, 0x00, 0x00, 0x00, 0x80, 0x3F, 0x00, 0x00, 0x80, 0xBF, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0xBF, 0x00, 0x00, 0x80, 0x3F,
            0x00, 0x00, 0x80, 0x3F, 0x00, 0x00, 0x80, 0x3F, 0x00, 0x00, 0x80, 0x3F, 0x00, 0x00,
            0x80, 0x3F, 0x00, 0x00, 0x80, 0x3F, 0x00, 0x00, 0x80, 0x3F, 0x00, 0x00, 0x80, 0x3F,
            0x00, 0x00, 0x80, 0x3F, 0x00, 0x00, 0x80, 0x3F, 0x00, 0x00, 0x80, 0x3F, 0x00, 0x00,
            0x80, 0x3F, 0x00, 0x00, 0x80, 0x3F, 0x00, 0x00, 0x80, 0x3F, 0x00, 0x00, 0x80, 0x3F,
            0x00, 0x00, 0x80, 0x3F, 0x00, 0x00, 0x80, 0x3F,
        ];

        (frm, &DATA)
    }

    fn data_v1_1() -> (Frm, &'static [u8]) {
        let frm = Frm {
            version: FrmVersion::V1_1,
            frames: vec![
                Frame {
                    option: 0,
                    plus_x: 1.,
                    pos_y: -1.,
                    bones: vec![[[0.; 4], [0.; 4], [0.; 4], [0.; 4]]],
                },
                Frame {
                    option: 0,
                    plus_x: -1.,
                    pos_y: 1.,
                    bones: vec![[[1.; 4], [1.; 4], [1.; 4], [1.; 4]]],
                },
            ],
            pos_z: vec![0., 1.],
        };

        const DATA: [u8; 170] = [
            0x46, 0x72, 0x6D, 0x20, 0x56, 0x65, 0x72, 0x20, 0x31, 0x2E, 0x31, 0x00, 0x02, 0x00,
            0x01, 0x00, 0x00, 0x00, 0x00, 0x80, 0x3F, 0x00, 0x00, 0x80, 0xBF, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0xBF, 0x00, 0x00, 0x80, 0x3F,
            0x00, 0x00, 0x80, 0x3F, 0x00, 0x00, 0x80, 0x3F, 0x00, 0x00, 0x80, 0x3F, 0x00, 0x00,
            0x80, 0x3F, 0x00, 0x00, 0x80, 0x3F, 0x00, 0x00, 0x80, 0x3F, 0x00, 0x00, 0x80, 0x3F,
            0x00, 0x00, 0x80, 0x3F, 0x00, 0x00, 0x80, 0x3F, 0x00, 0x00, 0x80, 0x3F, 0x00, 0x00,
            0x80, 0x3F, 0x00, 0x00, 0x80, 0x3F, 0x00, 0x00, 0x80, 0x3F, 0x00, 0x00, 0x80, 0x3F,
            0x00, 0x00, 0x80, 0x3F, 0x00, 0x00, 0x80, 0x3F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x80, 0x3F,
        ];

        (frm, &DATA)
    }
}
