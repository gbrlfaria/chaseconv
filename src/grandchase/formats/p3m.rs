use std::io::{Cursor, Result, Seek, SeekFrom};

use byteorder::{ReadBytesExt, WriteBytesExt, LE};

const NULL_BONE: u8 = 255;
const HEADER: &'static str = "Perfact 3D Model (Ver 0.5)\0";
const TEXTURE_NAME_LEN: usize = 260;

pub struct P3m {
    pub header: String,
    pub position_bones: Vec<PositionBone>,
    pub angle_bones: Vec<AngleBone>,
    pub texture_name: String,
    pub faces: Vec<[u16; 3]>,
    pub skin_vertices: Vec<SkinVertex>,
    pub mesh_vertices: Vec<MeshVertex>,
}

impl P3m {
    pub fn new() -> Self {
        Self {
            header: String::from(HEADER),
            position_bones: Vec::new(),
            angle_bones: Vec::new(),
            texture_name: String::new(),
            faces: Vec::new(),
            skin_vertices: Vec::new(),
            mesh_vertices: Vec::new(),
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut reader = Cursor::new(bytes);

        let mut p3m = Self::new();

        p3m.header = util::read_string(&mut reader, HEADER.len())?;
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

        p3m.texture_name = util::read_string(&mut reader, TEXTURE_NAME_LEN)?;

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

        Ok(Self::new())
    }

    fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();

        util::write_string(&mut bytes, &self.header, HEADER.len())?;
        bytes.write_u8(self.position_bones.len() as u8)?;
        bytes.write_u8(self.angle_bones.len() as u8)?;

        for position_bone in &self.position_bones {
            position_bone.into_bytes(&mut bytes)?;
        }
        for angle_bone in &self.angle_bones {
            angle_bone.into_bytes(&mut bytes)?;
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
            skin_vertex.into_bytes(&mut bytes)?;
        }
        for mesh_vertex in &self.mesh_vertices {
            mesh_vertex.into_bytes(&mut bytes)?;
        }

        Ok(bytes)
    }
}

pub struct PositionBone {
    position: [f32; 3],
    children: Vec<u8>,
}

impl PositionBone {
    fn new() -> Self {
        Self {
            position: [0.; 3],
            children: Vec::new(),
        }
    }

    fn from_reader(reader: &mut Cursor<&[u8]>) -> Result<Self> {
        let mut position_bone = Self::new();

        reader.read_f32_into::<LE>(&mut position_bone.position)?;

        for _ in 0..10 {
            let child = reader.read_u8()?;
            if child != NULL_BONE {
                position_bone.children.push(child);
            }
        }

        // Skip 2-byte struct alignment padding.
        reader.seek(SeekFrom::Current(2))?;

        Ok(position_bone)
    }

    fn into_bytes(&self, bytes: &mut Vec<u8>) -> Result<()> {
        for &index in &self.position {
            bytes.write_f32::<LE>(index)?;
        }

        for x in 0..10 {
            if x < self.children.len() {
                bytes.write_u8(self.children[x])?;
            } else {
                bytes.write_u8(NULL_BONE)?;
            }
        }

        // Write 2-byte struct alignment padding.
        bytes.write_u16::<LE>(0xffff)?;

        Ok(())
    }
}

pub struct AngleBone {
    position: [f32; 3],
    scale: f32,
    children: Vec<u8>,
}

impl AngleBone {
    fn new() -> Self {
        Self {
            position: [0.; 3],
            scale: 0.,
            children: Vec::new(),
        }
    }

    fn from_reader(reader: &mut Cursor<&[u8]>) -> Result<Self> {
        let mut angle_bone = Self::new();

        reader.read_f32_into::<LE>(&mut angle_bone.position)?;
        angle_bone.scale = reader.read_f32::<LE>()?;

        for _ in 0..10 {
            let child = reader.read_u8()?;
            if child != NULL_BONE {
                angle_bone.children.push(child);
            }
        }

        // Skip 2-byte struct alignment padding.
        reader.seek(SeekFrom::Current(2))?;

        Ok(angle_bone)
    }

    fn into_bytes(&self, bytes: &mut Vec<u8>) -> Result<()> {
        for &coordinate in &self.position {
            bytes.write_f32::<LE>(coordinate)?;
        }
        bytes.write_f32::<LE>(self.scale)?;

        for x in 0..10 {
            if x < self.children.len() {
                bytes.write_u8(self.children[x])?;
            } else {
                bytes.write_u8(NULL_BONE)?;
            }
        }

        // Write 2-byte struct alignment padding.
        bytes.write_u16::<LE>(0xffff)?;

        Ok(())
    }
}

pub struct SkinVertex {
    position: [f32; 3],
    weight: f32,
    bone_index: u8,
    normal: [f32; 3],
    uv: [f32; 2],
}

impl SkinVertex {
    fn new() -> Self {
        Self {
            position: [0.; 3],
            weight: 1.,
            bone_index: NULL_BONE,
            normal: [0.; 3],
            uv: [0.; 2],
        }
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

    fn into_bytes(&self, bytes: &mut Vec<u8>) -> Result<()> {
        for &coordinate in &self.position {
            bytes.write_f32::<LE>(coordinate)?;
        }
        bytes.write_f32::<LE>(self.weight)?;

        bytes.write_u8(self.bone_index)?;
        bytes.write_u8(self.bone_index)?;
        bytes.write_u8(NULL_BONE)?;
        bytes.write_u8(NULL_BONE)?;

        for &component in &self.normal {
            bytes.write_f32::<LE>(component)?;
        }
        for &coordinate in &self.uv {
            bytes.write_f32::<LE>(coordinate)?;
        }

        Ok(())
    }
}

pub struct MeshVertex {
    position: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
}

impl MeshVertex {
    fn new() -> Self {
        Self {
            position: [0.; 3],
            normal: [0.; 3],
            uv: [0.; 2],
        }
    }

    fn from_reader(reader: &mut Cursor<&[u8]>) -> Result<Self> {
        let mut mesh_vertex = Self::new();

        reader.read_f32_into::<LE>(&mut mesh_vertex.position)?;
        reader.read_f32_into::<LE>(&mut mesh_vertex.normal)?;
        reader.read_f32_into::<LE>(&mut mesh_vertex.uv)?;

        Ok(mesh_vertex)
    }

    fn into_bytes(&self, bytes: &mut Vec<u8>) -> Result<()> {
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

mod util {
    use std::io::{Cursor, Error, ErrorKind, Read, Result, Write};

    use byteorder::WriteBytesExt;

    pub fn read_string(reader: &mut Cursor<&[u8]>, length: usize) -> Result<String> {
        let mut bytes = vec![0 as u8; length];
        reader.read_exact(&mut bytes)?;

        match String::from_utf8(bytes) {
            Err(error) => Err(Error::new(ErrorKind::Other, error.to_string())),
            Ok(string) => Ok(string),
        }
    }

    pub fn write_string(bytes: &mut Vec<u8>, string: &str, max_len: usize) -> Result<()> {
        let len = usize::min(string.len(), max_len);
        bytes.write(string[0..len].as_bytes())?;

        // Set the remaining bytes to zero, if any.
        for _ in 0..(max_len - len) {
            bytes.write_u8(0)?;
        }

        Ok(())
    }
}
