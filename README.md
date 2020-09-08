# rdm4: Experimental rdm ⇄ glTF 2.0 Converter 

## command-line interface rdm4-bin

```
rdm4-bin v0.2-alpha
lukts30 <https://github.com/lukts30/rdm4>

USAGE:
    rdm4-bin.exe [FLAGS] [OPTIONS] --file <glTF or rdm FILE>

FLAGS:
    -g, --gltf         Convert from glTF to .rdm
    -s, --skeleton     Export (available) skin
    -a, --animation    Export (available) animation. RDM to glTF needs external animation file (rdanimation)
    -h, --help         Prints help information
    -v, --verbose      A level of verbosity, and can be used multiple times
    -V, --version      Prints version information

OPTIONS:
    -t, --diffusetexture <*.dds>...    DiffuseTextures
    -m, --rdanimation <anim/*.rdm>     External animation file for rdm
    -f, --file <glTF or rdm FILE>      Input file
```

## Example usage rdm 🠚 glTF 2.0
```console
$ ./rdm4-bin.exe --file rdm/container_ship_tycoons_lod1.rdm
```

## Usage with animation
```console
$ ./rdm4-bin.exe --file rdm/container_ship_tycoons_lod1.rdm --skeleton --animation --rdanimation anim/container_ship_tycoons_idle01.rdm
```
Can be shortened to:
```console
$ ./rdm4-bin.exe -f rdm/container_ship_tycoons_lod1.rdm -sam anim/container_ship_tycoons_idle01.rdm
[2020-08-25T21:48:17Z INFO  rdm4_bin] Using input file: "rdm/container_ship_tycoons_lod1.rdm"
[2020-08-25T21:48:17Z INFO  rdm4_bin] Export skeleton: true
[2020-08-25T21:48:17Z INFO  rdm4_bin] Export rdanimation: Some("anim/container_ship_tycoons_idle01.rdm")
[2020-08-25T21:48:17Z INFO  rdm4lib] loaded "rdm/container_ship_tycoons_lod1.rdm" into buffer
[2020-08-25T21:48:17Z INFO  rdm4lib] buffer size: 211286
[2020-08-25T21:48:17Z INFO  rdm4lib] off : 665
[2020-08-25T21:48:17Z INFO  rdm4lib] Read 6375 vertices of type P4h_N4b_G4b_B4b_T2h_I4b (28 bytes)
[2020-08-25T21:48:17Z INFO  rdm4lib::rdm_anim] loaded "anim/container_ship_tycoons_idle01.rdm" into buffer
[2020-08-25T21:48:17Z INFO  rdm4lib::rdm_anim] buffer size: 4862
[2020-08-25T21:48:17Z INFO  rdm4lib::rdm_anim] target model name: Lod1
[2020-08-25T21:48:17Z INFO  rdm4lib::rdm_anim] joint_targets_count: 3
[2020-08-25T21:48:17Z INFO  rdm4lib::rdm_anim] jtable: [(481, 506), (578, 607), (2535, 2558)]
[2020-08-25T21:48:17Z INFO  rdm4_bin] Skin and anim added !
[2020-08-25T21:48:17Z INFO  rdm4_bin] running gltf_export ...
```

## Usage with animation & diffuse texture (must match \*.cfg `cModelDiffTex` order) [**requires texconv**]
```console
$ ./rdm4-bin.exe -f rdm/resident_tier03_work.rdm -sam anim/resident_tier03_work_friendly_talk.rdm -t maps/resident_tier03_diff_0.dds ../resident_tier03/maps/resident_tier03_diff_0.dds
```

## Example usage glTF 2.0 🠚 rdm
**Flag --gltf or the alias -g must be used !**
<details>
<summary>Click to expand</summary>

```console
$ ./rdm4-bin.exe -f untitled.gltf -gsa
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

---
## Current limitations
- normals & tangent & bitangent contain placeholder garbage values.
- material or texture or shader id bytes were not yet "investigated"
- while glTF files can contain multiple animation this is not handled at all.
    - e.g cannot export "idle" and "work" into one glTF file
    - glTF animations json nodes could be cut out to generate all rdanimation files (workaround)
- [glTF 2.0 🠚 rdm](https://github.com/lukts30/rdm4/pull/4)
    - limited to glTF files that contain only **one** mesh and only **one** animation
    - channel.path: `translation` and `rotation` are supported. 
    - `scale` and `weights` are totally unsupported and are ignored ! To my knowledge impossible to implement since rdanimation "units" are 32 bytes large = 4\*4 rotation + 3\*4 translation + 1\*4 time
    - Must not require interpolation e.g. rotation t=[0,2,6] while translation t=[0,7]. Bypassed by using Blenders 'Always Sample Animation' export option.