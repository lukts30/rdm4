# rdm4: rdm ⇄ glTF 2.0 Converter 

## Windows Explorer right-click menu

It is possible to use parts of the converter functionality via a context menu in Windows Explorer. Not all features of the converter are available in this context menu notably non of the animation support (if you want to convert animations you must use the command-line interface!).

Create the folder C:\tools. Copy rdm4-bin.exe to C:\tools. Choose one of the reg files. Save it and apply it.

- [rdm4.reg](https://gist.github.com/lukts30/f38f1b9675a084c869e8a59c746d2504#file-rdm4-reg): Abort/Crashes if a output file already exist.
- [rdm4-force.reg](https://gist.github.com/lukts30/f38f1b9675a084c869e8a59c746d2504#file-rdm4_force-reg): Will override any already existing output file.


<img src="https://user-images.githubusercontent.com/24390575/124466252-c1221000-dd96-11eb-8feb-6786d7730bc1.png" width=40% height=40%>

- [Youtube playlist with a few videos showing how to use the rdm4 converter to make it a bit easier to get started with the converter.](https://www.youtube.com/playlist?list=PLBsFrFg1v0yxRMV36SkJRnSh_O-hMTsr2)

## command-line interface rdm4-bin

```
rdm4-bin 0.8.0-alpha.1
lukts30 <https://github.com/lukts30/rdm4>

USAGE:
    rdm4-bin [OPTIONS] --input <glTF or rdm FILE>

OPTIONS:
    -i, --input <glTF or rdm FILE>    Input file
    -o, --outdst <OUT>                Output file or folder. If `--in-is-out-filename` is set this
                                      must be a folder!
    -n, --in-is-out-filename          Sets output to input file name
        --force                       Override existing files
    -s, --skeleton                    Export (available) skin
    -a, --animation                   Export (available) animation. RDM to glTF needs external
                                      animation file (rdanimation)
    -v, --verbose                     A level of verbosity, and can be used multiple times
    -h, --help                        Print help information
    -V, --version                     Print version information

GLTF TO RDM OPTIONS:
    -g, --gltf <VertexFormat>
            VertexFormat for output rdm: P4h_N4b_G4b_B4b_T2h | P4h_N4b_G4b_B4b_T2h_I4b |
            P4h_N4b_G4b_B4b_T2h_I4b_W4b | P3f_N3f_G3f_B3f_T2f_C4b

        --gltf-mesh-index <GLTF_MESH_INDEX>
            glTF mesh index to convert to rdm [default: 0]

        --no_transform
            glTF to rdm: Do not apply node transforms. Recommended to use when working with
            animations

        --negative-x-and-v0v2v1
            Mirrors the object on the x axis

        --overide-mesh-idx <OVERIDE_MESH_IDX>
            Overrides MeshInstance mesh indcies. Useful to match the material order of an existing
            cfg

    -u, --gltf-node-joint-name-src <GLTF_NODE_JOINT_NAME_SRC>
            For glTF joint to rdm bone: source for a unique identifier: "UnstableIndex" |
            "UniqueName" [default: UniqueName]

RDM TO GLTF OPTIONS:
    -e, --gltf-export-format <GLTF_EXPORT_FORMAT>
            Export format to use for rdm to gltf: "glb", "gltf", "gltfmin" [default: glb]

    -m, --rdanimation <anim/*.rdm>
            External animation file for rdm

    -t, --diffusetexture <*.dds>
            DiffuseTextures
```

## Example usage (rdm 🠚 glTF 2.0)
```console
$ ./rdm4-bin.exe --input rdm/container_ship_tycoons_lod1.rdm
```

### Usage with animation (rdm 🠚 glTF 2.0)
```console
$ ./rdm4-bin.exe --input rdm/container_ship_tycoons_lod1.rdm --skeleton --animation --rdanimation anim/container_ship_tycoons_idle01.rdm
```
Can be shortened to:
```console
$ ./rdm4-bin.exe -i rdm/container_ship_tycoons_lod1.rdm -sam anim/container_ship_tycoons_idle01.rdm
```

## Example usage glTF 2.0 🠚 rdm
**Flag --gltf or the alias -g must be used! See the section on vertex formats below**
- **Note**: the example given here uses `-g=P4h_N4b_G4b_B4b_T2h_I4b_W4b` and `-sa` since it converts an animated glTF to rdm with anim files.
<details>
<summary>Click to expand</summary>

```console
$ ./rdm4-bin.exe -g=P4h_N4b_G4b_B4b_T2h_I4b_W4b -i untitled.gltf -sa
[2020-08-25T22:29:12Z INFO  rdm4_bin] Using input file: "untitled.gltf"
[2020-08-25T22:29:12Z INFO  rdm4_bin] Export skelton: true
[2020-08-25T22:29:12Z INFO  rdm4_bin] Export rdanimation: None
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] Mesh #0
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] - Primitive #0
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] verts_vec.len 5184
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] skin #0
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #_rootJoint |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #_rootJoint |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #_rootJoint |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:Hips_02 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:Hips_02 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:Hips_02 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:Spine_062 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:Spine_062 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:Spine_062 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:Spine1_063 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:Spine1_063 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:Spine1_063 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:Spine2_064 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:Spine2_064 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:Spine2_064 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:Neck_032 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:Neck_032 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:Neck_032 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:Head_00 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:Head_00 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:Head_00 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:HeadTop_End_01 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:HeadTop_End_01 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:HeadTop_End_01 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftShoulder_028 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftShoulder_028 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftShoulder_028 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftArm_03 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftArm_03 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftArm_03 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftForeArm_05 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftForeArm_05 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftForeArm_05 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHand_06 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHand_06 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHand_06 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandThumb1_023 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandThumb1_023 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandThumb1_023 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandThumb2_024 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandThumb2_024 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandThumb2_024 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandThumb3_025 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandThumb3_025 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandThumb3_025 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandThumb4_026 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandThumb4_026 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandThumb4_026 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandIndex1_07 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandIndex1_07 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandIndex1_07 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandIndex2_08 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandIndex2_08 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandIndex2_08 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandIndex3_09 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandIndex3_09 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandIndex3_09 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandIndex4_010 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandIndex4_010 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandIndex4_010 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandMiddle1_011 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandMiddle1_011 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandMiddle1_011 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandMiddle2_012 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandMiddle2_012 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandMiddle2_012 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandMiddle3_013 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandMiddle3_013 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandMiddle3_013 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandMiddle4_014 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandMiddle4_014 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandMiddle4_014 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandRing1_019 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandRing1_019 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandRing1_019 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandRing2_020 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandRing2_020 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandRing2_020 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandRing3_021 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandRing3_021 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandRing3_021 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandRing4_022 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandRing4_022 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandRing4_022 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandPinky1_015 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandPinky1_015 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandPinky1_015 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandPinky2_016 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandPinky2_016 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandPinky2_016 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandPinky3_017 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandPinky3_017 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandPinky3_017 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandPinky4_018 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandPinky4_018 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftHandPinky4_018 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightShoulder_058 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightShoulder_058 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightShoulder_058 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightArm_033 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightArm_033 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightArm_033 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightForeArm_035 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightForeArm_035 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightForeArm_035 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHand_036 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHand_036 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHand_036 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandThumb1_053 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandThumb1_053 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandThumb1_053 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandThumb2_054 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandThumb2_054 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandThumb2_054 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandThumb3_055 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandThumb3_055 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandThumb3_055 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandThumb4_056 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandThumb4_056 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandThumb4_056 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandIndex1_037 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandIndex1_037 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandIndex1_037 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandIndex2_038 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandIndex2_038 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandIndex2_038 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandIndex3_039 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandIndex3_039 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandIndex3_039 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandIndex4_040 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandIndex4_040 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandIndex4_040 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandMiddle1_041 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandMiddle1_041 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandMiddle1_041 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandMiddle2_042 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandMiddle2_042 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandMiddle2_042 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandMiddle3_043 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandMiddle3_043 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandMiddle3_043 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandMiddle4_044 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandMiddle4_044 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandMiddle4_044 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandRing1_049 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandRing1_049 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandRing1_049 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandRing2_050 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandRing2_050 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandRing2_050 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandRing3_051 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandRing3_051 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandRing3_051 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandRing4_052 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandRing4_052 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandRing4_052 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandPinky1_045 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandPinky1_045 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandPinky1_045 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandPinky2_046 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandPinky2_046 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandPinky2_046 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandPinky3_047 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandPinky3_047 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandPinky3_047 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandPinky4_048 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandPinky4_048 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightHandPinky4_048 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftUpLeg_031 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftUpLeg_031 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftUpLeg_031 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftLeg_027 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftLeg_027 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftLeg_027 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftFoot_04 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftFoot_04 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftFoot_04 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftToeBase_029 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftToeBase_029 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftToeBase_029 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftToe_End_030 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftToe_End_030 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:LeftToe_End_030 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightUpLeg_061 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightUpLeg_061 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightUpLeg_061 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightLeg_057 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightLeg_057 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightLeg_057 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightFoot_034 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightFoot_034 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightFoot_034 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightToeBase_059 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightToeBase_059 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightToeBase_059 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightToe_End_060 |  Translation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightToe_End_060 |  Rotation
[2020-08-25T22:29:12Z INFO  rdm4lib::gltf_reader] channel #mixamorig:RightToe_End_060 |  Scale
[2020-08-25T22:29:12Z WARN  rdm4lib::gltf_reader] output sampler not supported: 'Scale'
```

</details>

## Setting Vertex Formats for glTF 2.0 🠚 rdm

**-g sets your vertex format

### `P4h_N4b_G4b_B4b_T2h`: Vertex Format for standard meshes
### `P4h_N4b_G4b_B4b_T2h_I4b`: Vertex Format with unweighted Joints
- This needs at least Joint Data exported to the gltf.
### `P4h_N4b_G4b_B4b_T2h_I4b_W4b`: Vertex Format with weighted Joints, i.e. Portraits
- This needs Joint and Weight Data exported to the gltf.
### `P3f_N3f_G3f_B3f_T2f_C4b`: Cloth
- Needs vertex colors in the export. If you don't have an idea how to create them in blender, refer to [this video](https://www.youtube.com/watch?v=8mNk6r_bwxI)
### `P4h_T2h_C4b`: Decal Detail 
- Needs vertex colors in the export.
- Anno uses the vertex color for deviation in luminosity. 
- Use gray=0.5 as your standard color, then paint darker and brighter spots. 

> TLDR, if you just want a standard model, use `-g=P4h_N4b_G4b_B4b_T2h`!

---

## rdm4 current limitations

- gltf -> rdm: gltf file needs to include normals and **tangents**. they are not computed in the converter !
    - see [Blender glTF export](#Blender)
- glTF 2.0 🠚 rdm with animation
    - glTF node names are not necessarily unique but this converter uses them by default for rdm bone names. This might cause problems.
        - Use the option `--gltf-node-joint-name-src`.
        - [#50](https://github.com/lukts30/rdm4/issues/50)
    - Must not require interpolation e.g. rotation t=[0,2,6] while translation t=[0,7]. Bypassed by using Blenders 'Always Sample Animation' export option.
    - channel.path: `translation` and `rotation` are supported. 
        - channel.path: `scale` is unsupported! 
    - Morph Targets: `scale` and `weights` are unsupported! 
        - To my knowledge impossible to implement since rdanimation "units" are 32 bytes large = 4\*4 rotation + 3\*4 translation + 1\*4 time

---

# Blender 

## Export glTF vertex tangents with mesh 

- The exported glTF file must have tangents data. This is not the default option for the blender glTF exporter!
    - after selecting export on right side click on "geometry" and ENABLE "tangents".

<img src="https://user-images.githubusercontent.com/24390575/124466344-ddbe4800-dd96-11eb-93bf-d567b18eee5e.png" width=20% height=20%>

- Format can be glTF Binary/glTF Separate/glTF Embedded
    - There should not be a functional difference between these formats. They are equally supported by rdm4.
    - If you are unsure select __glTF Binary (.glb)__.
- Materials may optionally be enabled. If you are experiencing problems with textures test with and without.    