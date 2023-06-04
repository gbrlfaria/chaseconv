# Chaseconv

[![](https://img.shields.io/badge/version-v0.2.0-orange)](https://github.com/gabriel-dev/chaseconv/releases/latest)
[![](https://img.shields.io/github/downloads/gabriel-dev/chaseconv/latest/total)](https://github.com/gabriel-dev/chaseconv/releases/latest)
[![](https://img.shields.io/github/license/gabriel-dev/chaseconv)](./LICENSE)

Chaseconv is a fast and simple 3D asset converter for [Grand Chase](https://en.wikipedia.org/wiki/Grand_Chase). It can convert 3D assets from the game into GLTF files and vice-versa.

![](img/example.png)

## Download

You can download the application in the [Releases Page](https://github.com/gabriel-dev/chaseconv/releases/latest).

## Usage

Using the program is straightforward:

1. Drag and drop the desired files onto `chaseconv.exe` (they should belong to the same model!).
2. Select the format you want to convert the files into.
3. Select the output folder.

Here's a small demonstration:

![](img/tutorial.gif)

## Limitations

There are limitations, however:

### Exporting

- You can't export a standalone animation to GLTF. You can only export animations alongside models because joint data is stored inside P3M files.
- Some animations may lose data when being exported. That's because some models have fewer joints than their animations. So the extra animation channels end up being discarded.

### Importing

- All bones of the model should be named "bone_X", where X is the index of the bone (e.g., "bone_0", "bone_1", ...). The root bone should be named "root".
- Bones should have no rotation in the bind pose. When importing GLTF files into Blender, make sure to set the bone direction configuration to "Blender".
- Regarding animations, the bones of the model only support rotations, whereas the root bone only supports translations.
- Only the first skeleton/skin in each GLTF file will be taken into account.

## Contributing

- Found a bug? Please create an issue describing the problem.

- Looking for a specific feature? Feel free to leave a pull request. Some opportunities for new features are:
  - Add a command-line interface (CLI) to make mass-exporting files feasible.
  - Add support for other data formats (COLLADA, OBJ, etc.).
  - Relax some of the restrictions on the imported models.
