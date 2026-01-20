The Rust ecosystem offers **at least 25 libraries** for signed distance function work, spanning general 3D modeling, text rendering, and GPU-accelerated implementations. **Fidget** and **sdfu** stand out as the most capable pure-Rust options, while **libfive** bindings provide access to battle-tested C++ infrastructure. For real-time rendering, **bevy_smud** leads with 100,000 shapes at 60fps performance.

## General-purpose SDF modeling libraries

These libraries form the core of Rust's SDF ecosystem, offering primitives, CSG operations, and transformations for creating complex 3D shapes.

### sdfu — the Inigo Quilez standard

| Attribute | Details |
| --- | --- |
| **Crates.io** | https://crates.io/crates/sdfu |
| **GitHub** | https://github.com/fu5ha/sdfu |
| **Version** | 0.3.0 |
| **Last updated** | ~2019 (stable, minimal maintenance) |
| **GPU support** | CPU-only |

**sdfu** is the most faithful Rust implementation of Inigo Quilez's SDF techniques, making it the go-to choice for ray-marching renderers. It provides comprehensive primitives (sphere, box, capsule, cylinder, torus), full CSG operations with smooth blending via `.union_smooth()`, and transformation modifiers for translate, rotate, and scale. The library supports multiple math backends including **ultraviolet**, **nalgebra**, and **vek** through its generic trait system.

The main limitation is lack of active development—the codebase hasn't seen significant updates since 2019. However, its API is stable and well-suited for offline rendering and path tracing applications. Documentation covers only 43% of the codebase.

### Fidget — near-GPU performance through JIT compilation

| Attribute | Details |
| --- | --- |
| **Crates.io** | https://crates.io/crates/fidget |
| **GitHub** | https://github.com/mkeeter/fidget |
| **Stars** | 398 |
| **Last updated** | Active (2025) |
| **GPU support** | CPU with native JIT (competitive with GPU) |

**Fidget** represents the cutting edge of CPU-based SDF evaluation. Created by Matthew Keeter (author of libfive), it uses hand-written **aarch64 and x86_64 JIT routines** to achieve performance nearly matching GPU-based Massively Parallel Rendering. The library converts expression graphs to optimized tapes, applies interval arithmetic for early rejection, and supports SIMD evaluation.

Key features include Manifold Dual Contouring for mesh generation, Rhai scripting integration, and WebAssembly support (interpreter mode). It requires AVX2 on x86_64 platforms. Fidget excels for CAD applications, scientific visualization, and any scenario where you need GPU-class performance without actual GPU dependencies.

### implicit3d — rounded CSG and special deformers

| Attribute | Details |
| --- | --- |
| **Crates.io** | https://crates.io/crates/implicit3d |
| **GitHub** | https://github.com/hmeyer/implicit3d |
| **Version** | 0.14.2 |
| **Last updated** | Low activity (maintained as needed) |
| **GPU support** | CPU-only |

This library stands out for its **rounded CSG operations** with configurable blend radius parameters, enabling smooth transitions between boolean operations. It includes unique deformers like `Bender` (bends around Z-axis) and `Twister` (twists along Z-axis). Part of the truescad ecosystem, it's designed for dual contouring mesh generation.

Primitives include sphere, cylinder, cone, and plane variants. The mesh-based SDF generation is noted as "horribly inefficient" for complex meshes. Uses nalgebra 0.22 (older version). Documentation coverage is 100%.

### csgrs — the actively maintained OpenSCAD alternative

| Attribute | Details |
| --- | --- |
| **Crates.io** | https://crates.io/crates/csgrs |
| **GitHub** | https://github.com/timschmidt/csgrs |
| **Stars** | 176 |
| **Version** | 0.20.1 (July 2025) |
| **Last updated** | Very active (952 commits) |
| **GPU support** | CPU-only (GPU planned) |

**csgrs** offers the most extensive feature set with OpenSCAD-like syntax. Beyond standard CSG, it provides **metaballs**, **gyroid** and **Schwarz P/D surfaces**, distribution operations for arrays, and import/export for STL/DXF formats. The `Mesh::sdf<F>()` method generates meshes from arbitrary SDF functions using marching cubes.

The library integrates with the Dimforge ecosystem (nalgebra, Parry, Rapier), supports multithreading via rayon, and offers Bevy mesh export. While not a pure SDF library (it focuses on mesh representation), it bridges SDF modeling with practical CAD workflows for 3D printing and CNC toolpath generation.

### saft — Embark Studios' interpreter approach

| Attribute | Details |
| --- | --- |
| **Crates.io** | https://crates.io/crates/saft |
| **GitHub** | (Embark Studios internal) |
| **Last updated** | Active |
| **GPU support** | CPU + GLSL code generation |

Developed by Embark Studios, **saft** provides an SDF function interpreter and compiler with marching cubes meshing. It generates GLSL code for the interpreter, bridging CPU-based authoring with GPU rendering. The primitive library covers sphere, box, capsule, cone, and torus, with smooth CSG variants and material/color support.

## SDF text and font rendering libraries

Font rendering using SDFs enables resolution-independent text with effects like outlines, glows, and shadows. The Rust ecosystem offers both C++ bindings and pure implementations.

### msdfgen — mature C++ bindings for MSDF

| Attribute | Details |
| --- | --- |
| **Crates.io** | https://crates.io/crates/msdfgen |
| **GitHub** | https://github.com/katyo/msdfgen-rs |
| **Last updated** | ~2022 |
| **GPU support** | CPU generation, GPU rendering |

These bindings wrap Viktor Chlumský's msdfgen library, supporting SDF, PSDF, **MSDF** (multi-channel), and **MTSDF** (with true distance). Integration options include ttf-parser, font crate, and freetype-rs for font loading. The multi-channel approach dramatically improves corner sharpness compared to single-channel SDFs.

### fdsm — pure Rust MSDF implementation

| Attribute | Details |
| --- | --- |
| **Crates.io** | https://crates.io/crates/fdsm |
| **Repository** | https://gitlab.com/Kyarei/fdsm |
| **Version** | 0.8.0 |
| **Documentation** | 85.52% coverage |
| **GPU support** | CPU-only |

**fdsm** reimplements MSDF generation entirely in Rust following Chlumský's master thesis algorithm. It eliminates C++ dependencies while providing sign correction and visualization helpers. The separate `fdsm-ttf-parser` crate handles TTF integration. Roadmap includes error correction and benchmarks against the original msdfgen.

### kaku — wgpu text rendering with SDF caching

| Attribute | Details |
| --- | --- |
| **Crates.io** | https://crates.io/crates/kaku |
| **GitHub** | https://github.com/villuna/kaku |
| **Version** | 0.1.1 (July 2024) |
| **GPU support** | wgpu (generation ~1ms/glyph CPU, rendering GPU) |

The only **wgpu-integrated SDF text solution**, kaku caches distance fields per glyph per font, enabling fast outlining effects. It loads OpenType fonts via ab_glyph and supports pre-computation of distance fields. Non-SDF rendering is available when performance trumps quality. The codebase is 92.3% Rust, 7.7% WGSL.

### Additional text libraries worth noting

- **fontsdf** (https://crates.io/crates/fontsdf): Fontdue extension generating SDFs directly without raster downscaling, `no_std` compatible, supports dual distance fields for the Valve SIGGRAPH 2007 technique
- **easy-signed-distance-field** (https://github.com/gabdube/easy-signed-distance-field): Works with raw line inputs and TTF/OTF fonts, ~2ms per character at 64px, WebGL texture upload support
- **blurry** (https://crates.io/crates/blurry): Generates SDF glyph atlases with metadata, updated within the last two weeks
- **sdf_font_tools** (https://github.com/stadiamaps/sdf_font_tools): Suite for MapLibre/Mapbox GL compatible Protobuf-encoded font rendering

## GPU-accelerated and real-time rendering

For applications requiring real-time performance, several libraries leverage GPU compute through various shader compilation approaches.

### bevy_smud — 100k shapes at 60fps

| Attribute | Details |
| --- | --- |
| **Crates.io** | https://crates.io/crates/bevy_smud |
| **GitHub** | https://github.com/johanhelsing/bevy_smud |
| **Stars** | 170 |
| **Version** | 0.12.0 (October 2025) |
| **GPU API** | wgpu via Bevy |
| **Shader language** | WGSL |

The premier **2D SDF plugin for Bevy**, bevy_smud handles runtime shader generation with Inigo Quilez's primitives ported to WGSL (sd_circle, sd_box, sd_ellipse, etc.). Performance benchmarks show **100,000 shapes at 60fps** with 40 different shape/fill combinations. It includes a picking backend with SDF-based hit detection. Actively maintained for Bevy 0.17.

### rust-gpu — the foundation for Rust shaders

| Attribute | Details |
| --- | --- |
| **GitHub** | https://github.com/EmbarkStudios/rust-gpu |
| **Stars** | 8,000+ |
| **Output** | SPIR-V bytecode |
| **Targets** | Vulkan, wgpu, OpenGL |

**rust-gpu** compiles Rust code directly to GPU shaders via `rustc_codegen_spirv`. This enables writing type-safe fragment, vertex, and compute shaders in Rust. The `spirv_std` crate provides GPU-specific functions. Multiple runner options exist: wgpu, Vulkan (ash), and CPU fallback. This project underlies most Rust-native GPU SDF work.

### rust-gpu-sdf — cross-platform CPU/GPU library

| Attribute | Details |
| --- | --- |
| **GitHub** | https://github.com/Bevy-Rust-GPU/rust-gpu-sdf |
| **Stars** | 15 |
| **Last updated** | ~2023 |

A `no_std` SDF library designed for both CPU evaluation in regular Rust and GPU execution via rust-gpu shaders. The architecture enables authoring SDF code once and running it on either platform, making it valuable for development workflows where you prototype on CPU and deploy to GPU.

### sdf2mesh — GPU-accelerated mesh generation

| Attribute | Details |
| --- | --- |
| **Crates.io** | https://crates.io/crates/sdf2mesh |
| **GPU API** | wgpu |

Converts SDFs to triangle meshes using dual contouring with **wgpu GPU acceleration**. Supports GLSL fragment shaders and ShaderToy integration, enabling direct mesh export from ShaderToy SDFs. Slice-by-slice processing keeps memory usage manageable. Outputs STL files for 3D printing.

### Claydash — experimental 3D SDF modeler

| Attribute | Details |
| --- | --- |
| **GitHub** | https://github.com/antoineMoPa/claydash |
| **Live demo** | https://app.claydash.com/ |
| **GPU API** | wgpu via Bevy |

An experimental web-accessible 3D SDF modeler built on Bevy. Features include transform operations (grab, scale, rotate, duplicate), color picking, and multi-selection. Boolean operations are planned. The live demo runs in-browser via WebGPU/WebGL.

## Bindings to C/C++ libraries

### libfive — parametric CAD infrastructure

| Attribute | Details |
| --- | --- |
| **Crates.io** | https://crates.io/crates/libfive |
| **Binds to** | libfive (Matthew Keeter's C++ library) |
| **Status** | Stable, actively generated |

High-level Rust bindings to libfive's functional representation modeling system. Provides tree operations, CSG, meshing, and contour extraction. The optional `stdlib` feature includes a standard library of shapes and transforms. Variables enable parameterized models for generative design and mass customization. This represents the most mature option for serious CAD work.

## Comparison and selection guide

| Library | Primitives | CSG | Smooth blend | GPU | Focus | Maintenance |
| --- | --- | --- | --- | --- | --- | --- |
| **Fidget** | ✅ | ✅ | ✅ | JIT ≈ GPU | CAD/scientific | Very active |
| **sdfu** | ✅ | ✅ | ✅ | ❌ | Ray-marching | Stable/dormant |
| **libfive** | ✅ | ✅ | ✅ | ❌ | CAD | Active |
| **csgrs** | ✅ | ✅ | Mesh-based | Planned | 3D printing | Very active |
| **bevy_smud** | ✅ 2D | Limited | ✅ | ✅ wgpu | Real-time 2D | Active |
| **saft** | ✅ | ✅ | ✅ | GLSL gen | Games | Active |
| **implicit3d** | ✅ | ✅ | Rounded | ❌ | Mesh gen | Low |

For **ray-marching and offline rendering**, sdfu remains the cleanest API despite limited maintenance. For **CAD and parametric design**, Fidget or libfive bindings offer the most capability. For **game development with Bevy**, bevy_smud handles 2D while Claydash explores 3D territory. For **font rendering**, kaku provides wgpu integration while fdsm offers pure-Rust MSDF generation.

## Conclusion

The Rust SDF ecosystem has matured significantly, with **Fidget's JIT compilation** representing a breakthrough in CPU-based performance and **bevy_smud** proving that pure-Rust GPU rendering can handle production workloads. The gap between Rust-native and C++ binding approaches is closing—fdsm nearly matches msdfgen, and Fidget's author (Matthew Keeter) has effectively ported libfive's concepts to idiomatic Rust.

Key trends include WGSL adoption for cross-platform GPU work, increasing integration with the Bevy game engine, and growing interest in web deployment via WebAssembly. Notable gaps remain in GPU-accelerated 3D CSG (most solutions are still CPU-bound or 2D-only) and unified tooling for the SDF-to-mesh-to-print pipeline. For projects starting today, Fidget deserves serious consideration for its performance characteristics, while the libfive bindings offer the most battle-tested foundation for production CAD applications.