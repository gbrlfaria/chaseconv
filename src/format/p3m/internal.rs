use std::io::{Cursor, Seek, SeekFrom};

use anyhow::Result;
use byteorder::{ReadBytesExt, WriteBytesExt, LE};

// The typo is intentional and follows the string used in the official assets.
const VERSION_HEADER: &str = "Perfact 3D Model (Ver 0.5)\0";
const INVALID_BONE_INDEX: u8 = 255;
const TEXTURE_NAME_LEN: usize = 260;

/// Represents a P3M file. The P3M format stores geometry data from GrandChase, including mesh,
/// bone hierarchy, and skinning. It uses the left-handed coordinate system (Y-up).
#[derive(Debug, PartialEq)]
pub struct P3m {
    /// The default version header for the P3M format.
    pub version_header: String,
    /// The list of bone translations.
    pub position_bones: Vec<PositionBone>,
    /// The list of bone rotations. Together with position bones, they compose the bone hierarchy.
    pub angle_bones: Vec<AngleBone>,
    /// The name of the texture applied to the mesh. This field is unused and is always empty.
    pub texture_name: String,
    /// The list of faces of the polygon mesh. It's comprised of clockwise winded triangles.
    pub faces: Vec<[u16; 3]>,
    /// The polygon mesh vertices with skinning data.
    pub skin_vertices: Vec<SkinVertex>,
    /// The unskinned polygon mesh vertices. In practice, this field is unused.
    pub mesh_vertices: Vec<MeshVertex>,
}

impl P3m {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut reader = Cursor::new(bytes);

        let mut p3m = Self::new();

        p3m.version_header =
            util::read_string(&mut reader, VERSION_HEADER.len()).unwrap_or_default();
        let num_position_bones = reader.read_u8()?;
        let num_angle_bones = reader.read_u8()?;

        for _ in 0..num_position_bones {
            p3m.position_bones
                .push(PositionBone::from_reader(&mut reader)?);
        }
        for _ in 0..num_angle_bones {
            p3m.angle_bones.push(AngleBone::from_reader(&mut reader)?);
        }

        let num_vertices = reader.read_u16::<LE>()?;
        let num_faces = reader.read_u16::<LE>()?;

        p3m.texture_name = util::read_string(&mut reader, TEXTURE_NAME_LEN).unwrap_or_default();

        for _ in 0..num_faces {
            let mut face = [0; 3];
            reader.read_u16_into::<LE>(&mut face)?;
            p3m.faces.push(face);
        }
        for _ in 0..num_vertices {
            p3m.skin_vertices
                .push(SkinVertex::from_reader(&mut reader)?);
        }
        for _ in 0..num_vertices {
            p3m.mesh_vertices
                .push(MeshVertex::from_reader(&mut reader)?);
        }

        Ok(p3m)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();

        util::write_string(&mut bytes, &self.version_header, VERSION_HEADER.len())?;
        bytes.write_u8(self.position_bones.len() as u8)?;
        bytes.write_u8(self.angle_bones.len() as u8)?;

        for position_bone in &self.position_bones {
            position_bone.to_bytes(&mut bytes)?;
        }
        for angle_bone in &self.angle_bones {
            angle_bone.to_bytes(&mut bytes)?;
        }

        bytes.write_u16::<LE>(self.skin_vertices.len() as u16)?;
        bytes.write_u16::<LE>(self.faces.len() as u16)?;

        util::write_string(&mut bytes, &self.texture_name, TEXTURE_NAME_LEN)?;

        for face in &self.faces {
            for &index in face {
                bytes.write_u16::<LE>(index)?;
            }
        }
        for skin_vertex in &self.skin_vertices {
            skin_vertex.to_bytes(&mut bytes)?;
        }
        for mesh_vertex in &self.mesh_vertices {
            mesh_vertex.to_bytes(&mut bytes)?;
        }

        Ok(bytes)
    }
}

impl Default for P3m {
    fn default() -> Self {
        Self {
            // Remove the null terminator.
            version_header: String::from(&VERSION_HEADER[..VERSION_HEADER.len() - 1]),
            position_bones: Vec::new(),
            angle_bones: Vec::new(),
            texture_name: String::new(),
            faces: Vec::new(),
            skin_vertices: Vec::new(),
            mesh_vertices: Vec::new(),
        }
    }
}

/// A translation modifier that applies to a set of children angle bones.
#[derive(Debug, Clone, PartialEq)]
pub struct PositionBone {
    /// The translation applied to the children, relative to the parent bone.
    pub position: [f32; 3],
    /// The angle bones to which the translation applies. Up to ten children are supported.
    pub children: Vec<u8>,
}

impl PositionBone {
    pub fn new() -> Self {
        Default::default()
    }

    fn from_reader(reader: &mut Cursor<&[u8]>) -> Result<Self> {
        let mut position_bone = Self::new();

        reader.read_f32_into::<LE>(&mut position_bone.position)?;

        for _ in 0..10 {
            let child = reader.read_u8()?;
            if child != INVALID_BONE_INDEX {
                position_bone.children.push(child);
            }
        }

        // Skip 2-byte struct alignment padding.
        reader.seek(SeekFrom::Current(2))?;

        Ok(position_bone)
    }

    fn to_bytes(&self, bytes: &mut Vec<u8>) -> Result<()> {
        for &coordinate in &self.position {
            bytes.write_f32::<LE>(coordinate)?;
        }

        for x in 0..10 {
            if x < self.children.len() {
                bytes.write_u8(self.children[x])?;
            } else {
                bytes.write_u8(INVALID_BONE_INDEX)?;
            }
        }

        // Write 2-byte struct alignment padding.
        bytes.write_u16::<LE>(0xffff)?;

        Ok(())
    }
}

impl Default for PositionBone {
    fn default() -> Self {
        Self {
            position: [0.; 3],
            children: Vec::new(),
        }
    }
}

/// A rotation modifier that applies to a set of children position bones. All rotations are zeroed
/// by default.
/// These are the actual bones of the skeleton and what skin vertices and keyframe bone indices
/// refer to.
#[derive(Debug, PartialEq)]
pub struct AngleBone {
    /// This field is unused and is always zero.
    pub position: [f32; 3],
    /// This field is unused and is always zero.
    pub scale: f32,
    /// The position bones to which the rotations apply. Up to 10 children are supported.
    pub children: Vec<u8>,
}

impl AngleBone {
    pub fn new() -> Self {
        Default::default()
    }

    fn from_reader(reader: &mut Cursor<&[u8]>) -> Result<Self> {
        let mut angle_bone = Self::new();

        reader.read_f32_into::<LE>(&mut angle_bone.position)?;
        angle_bone.scale = reader.read_f32::<LE>()?;

        for _ in 0..10 {
            let child = reader.read_u8()?;
            if child != INVALID_BONE_INDEX {
                angle_bone.children.push(child);
            }
        }

        // Skip 2-byte struct alignment padding.
        reader.seek(SeekFrom::Current(2))?;

        Ok(angle_bone)
    }

    fn to_bytes(&self, bytes: &mut Vec<u8>) -> Result<()> {
        for &coordinate in &self.position {
            bytes.write_f32::<LE>(coordinate)?;
        }
        bytes.write_f32::<LE>(self.scale)?;

        for x in 0..10 {
            if x < self.children.len() {
                bytes.write_u8(self.children[x])?;
            } else {
                bytes.write_u8(INVALID_BONE_INDEX)?;
            }
        }

        // Write 2-byte struct alignment padding.
        bytes.write_u16::<LE>(0xffff)?;

        Ok(())
    }
}

impl Default for AngleBone {
    fn default() -> Self {
        Self {
            position: [0.; 3],
            scale: 0.,
            children: Vec::new(),
        }
    }
}

/// A skinned vertex of the mesh. Oficially, each vertex can only be influenced by a single bone,
/// always with max intensity.
#[derive(Debug, PartialEq)]
pub struct SkinVertex {
    /// Vertex position with the corresponding bone matrix applied.
    pub position: [f32; 3],
    /// Bone influence weight. In practice, it's unused and is always one.
    pub weight: f32,
    /// Index of the angle bone that influences the vertex **plus** the number of position
    /// bones.
    pub bone_index: u8,
    /// Vertex normal vector.
    pub normal: [f32; 3],
    /// UV texture coordinates.
    pub uv: [f32; 2],
}

impl SkinVertex {
    pub fn new() -> Self {
        Default::default()
    }

    fn from_reader(reader: &mut Cursor<&[u8]>) -> Result<Self> {
        let mut skin_vertex = Self::new();

        reader.read_f32_into::<LE>(&mut skin_vertex.position)?;
        skin_vertex.weight = reader.read_f32::<LE>()?;

        skin_vertex.bone_index = reader.read_u8()?;
        // Ignore unused bone indices.
        reader.seek(SeekFrom::Current(3))?;

        reader.read_f32_into::<LE>(&mut skin_vertex.normal)?;
        reader.read_f32_into::<LE>(&mut skin_vertex.uv)?;

        Ok(skin_vertex)
    }

    fn to_bytes(&self, bytes: &mut Vec<u8>) -> Result<()> {
        for &coordinate in &self.position {
            bytes.write_f32::<LE>(coordinate)?;
        }
        bytes.write_f32::<LE>(self.weight)?;

        bytes.write_u8(self.bone_index)?;
        bytes.write_u8(self.bone_index)?;
        bytes.write_u8(INVALID_BONE_INDEX)?;
        bytes.write_u8(INVALID_BONE_INDEX)?;

        for &component in &self.normal {
            bytes.write_f32::<LE>(component)?;
        }
        for &coordinate in &self.uv {
            bytes.write_f32::<LE>(coordinate)?;
        }

        Ok(())
    }
}

impl Default for SkinVertex {
    fn default() -> Self {
        Self {
            position: [0.; 3],
            weight: 1.,
            bone_index: INVALID_BONE_INDEX,
            normal: [0.; 3],
            uv: [0.; 2],
        }
    }
}

/// An unskinned vertex of the mesh.
#[derive(Debug, PartialEq)]
pub struct MeshVertex {
    // Vertex position without bone influence.
    pub position: [f32; 3],
    /// Vertex normal vector.
    pub normal: [f32; 3],
    /// UV texture coordinates.
    pub uv: [f32; 2],
}

impl MeshVertex {
    pub fn new() -> Self {
        Default::default()
    }

    fn from_reader(reader: &mut Cursor<&[u8]>) -> Result<Self> {
        let mut mesh_vertex = Self::new();

        reader.read_f32_into::<LE>(&mut mesh_vertex.position)?;
        reader.read_f32_into::<LE>(&mut mesh_vertex.normal)?;
        reader.read_f32_into::<LE>(&mut mesh_vertex.uv)?;

        Ok(mesh_vertex)
    }

    fn to_bytes(&self, bytes: &mut Vec<u8>) -> Result<()> {
        for &coordinate in &self.position {
            bytes.write_f32::<LE>(coordinate)?;
        }
        for &component in &self.normal {
            bytes.write_f32::<LE>(component)?;
        }
        for &coordinate in &self.uv {
            bytes.write_f32::<LE>(coordinate)?;
        }

        Ok(())
    }
}

impl Default for MeshVertex {
    fn default() -> Self {
        Self {
            position: [0.; 3],
            normal: [0.; 3],
            uv: [0.; 2],
        }
    }
}

mod util {
    use std::io::{Cursor, Error, ErrorKind, Read, Result, Write};

    use byteorder::WriteBytesExt;

    /// Reads certain amount of bytes into a string. The returned string gets truncated at the
    /// first null terminator in the byte sequence read, if there is any.
    pub fn read_string(reader: &mut Cursor<&[u8]>, max_len: usize) -> Result<String> {
        let mut bytes = vec![0; max_len];
        reader.read_exact(&mut bytes)?;

        // Truncate the string starting at the null terminator.
        let len = memchr::memchr(0, &bytes).unwrap_or(max_len);
        bytes.drain(len..);

        match String::from_utf8(bytes) {
            Ok(string) => Ok(string),
            Err(error) => Err(Error::new(ErrorKind::Other, error.to_string())),
        }
    }

    /// Writes a string with certain length in bytes. If the string is shorter than the maximum
    /// length allowed, the remaining bytes are filled with zero. If it's longer, it's truncated.
    pub fn write_string(bytes: &mut Vec<u8>, string: &str, max_len: usize) -> Result<()> {
        let len = usize::min(string.len(), max_len);
        bytes.write_all(string[0..len].as_bytes())?;

        // Set the remaining bytes to zero, if any.
        for _ in 0..(max_len - len) {
            bytes.write_u8(0)?;
        }

        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use pretty_assertions::assert_eq;

        use super::*;

        #[test]
        fn read_str_exact() {
            let bytes = b"Hi there!\x00";
            let mut reader = Cursor::new(&bytes[..]);

            assert_eq!(
                String::from("Hi there!"),
                read_string(&mut reader, bytes.len()).unwrap()
            );
            assert!(reader.position() == bytes.len() as u64);
        }

        #[test]
        fn read_str_shorter() {
            let bytes = b"Hello\x00, world";
            let mut reader = Cursor::new(&bytes[..]);

            assert_eq!(
                String::from("Hello"),
                read_string(&mut reader, bytes.len()).unwrap()
            );
            assert!(reader.position() == bytes.len() as u64);
        }

        #[test]
        fn read_str_invalid() {
            let bytes = b"\xf8\xa1\xa1\xa1\xa1";
            let mut reader = Cursor::new(&bytes[..]);

            assert!(read_string(&mut reader, bytes.len()).is_err());
            assert!(reader.position() == bytes.len() as u64);
        }

        #[test]
        fn write_str_shorter() {
            let mut bytes = Vec::new();
            write_string(&mut bytes, "Hello", 8).unwrap();

            assert_eq!(b"Hello\x00\x00\x00".to_vec(), bytes);
        }

        #[test]
        fn write_str_exact() {
            let mut bytes = Vec::new();
            write_string(&mut bytes, "Hi!", 3).unwrap();

            assert_eq!(b"Hi!".to_vec(), bytes);
        }

        #[test]
        fn write_str_longer() {
            let mut bytes = Vec::new();
            write_string(&mut bytes, "Hi there!", 2).unwrap();

            assert_eq!(b"Hi".to_vec(), bytes);
        }
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn read() {
        let (expected, bytes) = data();
        let actual = P3m::from_bytes(bytes).unwrap();

        assert_eq!(expected, actual);
    }

    #[test]
    fn write() {
        let (p3m, expected) = data();
        let actual = p3m.to_bytes().unwrap();

        assert_eq!(expected, actual);
    }

    fn data() -> (P3m, &'static [u8]) {
        let mut p3m = P3m::default();
        p3m.position_bones = vec![
            PositionBone {
                position: [0., 0., 0.],
                children: vec![0],
            },
            PositionBone {
                position: [1., 0., 0.],
                children: vec![1],
            },
        ];
        p3m.angle_bones = vec![
            AngleBone {
                position: [0., 0., 0.],
                scale: 0.,
                children: vec![1],
            },
            AngleBone {
                position: [0., 0., 0.],
                scale: 0.,
                children: Vec::new(),
            },
        ];
        p3m.faces = vec![[0, 1, 2]];
        p3m.skin_vertices = vec![
            SkinVertex {
                position: [1., 0., 0.],
                weight: 1.,
                bone_index: 0,
                uv: [0., 0.],
                normal: [1., 0., 0.],
            },
            SkinVertex {
                position: [0., 1., 0.],
                weight: 1.,
                bone_index: 0,
                uv: [0.5, 0.5],
                normal: [1., 0., 0.],
            },
            SkinVertex {
                position: [1., 0., 1.],
                weight: 1.,
                bone_index: 1,
                uv: [1., 1.],
                normal: [1., 0., 0.],
            },
        ];
        p3m.mesh_vertices = vec![
            MeshVertex {
                position: [1., 0., 0.],
                uv: [0., 0.],
                normal: [1., 0., 0.],
            },
            MeshVertex {
                position: [0., 1., 0.],
                uv: [0.5, 0.5],
                normal: [1., 0., 0.],
            },
            MeshVertex {
                position: [0., 0., 1.],
                uv: [1., 1.],
                normal: [1., 0., 0.],
            },
        ];

        const DATA: [u8; 619] = [
            0x50, 0x65, 0x72, 0x66, 0x61, 0x63, 0x74, 0x20, 0x33, 0x44, 0x20, 0x4d, 0x6f, 0x64,
            0x65, 0x6c, 0x20, 0x28, 0x56, 0x65, 0x72, 0x20, 0x30, 0x2e, 0x35, 0x29, 0x00, 0x02,
            0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x80,
            0x3f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x03, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x02, 0x00, 0x00, 0x00, 0x80,
            0x3f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x3f, 0x00,
            0x00, 0xff, 0xff, 0x00, 0x00, 0x80, 0x3f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x80, 0x3f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x3f, 0x00, 0x00, 0xff,
            0xff, 0x00, 0x00, 0x80, 0x3f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x3f, 0x00, 0x00, 0x00, 0x3f, 0x00, 0x00, 0x80, 0x3f, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x80, 0x3f, 0x00, 0x00, 0x80, 0x3f, 0x01, 0x01, 0xff, 0xff, 0x00,
            0x00, 0x80, 0x3f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80,
            0x3f, 0x00, 0x00, 0x80, 0x3f, 0x00, 0x00, 0x80, 0x3f, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x3f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x80, 0x3f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x3f, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x3f, 0x00, 0x00, 0x00, 0x3f, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x3f, 0x00, 0x00, 0x80,
            0x3f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x3f, 0x00,
            0x00, 0x80, 0x3f,
        ];

        (p3m, &DATA)
    }
}
