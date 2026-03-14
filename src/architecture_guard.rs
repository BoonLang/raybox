#[cfg(test)]
mod tests {
    use crate::{ui2d_shader_bindings, ui_physical_shader_bindings};
    use std::fs;
    use std::mem::{align_of, size_of};
    use std::path::{Path, PathBuf};

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn collect_rs_files(root: &Path, files: &mut Vec<PathBuf>) {
        let entries = fs::read_dir(root).unwrap_or_else(|err| {
            panic!("failed to read {}: {err}", root.display());
        });
        for entry in entries {
            let entry = entry.unwrap_or_else(|err| panic!("failed to read dir entry: {err}"));
            let path = entry.path();
            if path.is_dir() {
                collect_rs_files(&path, files);
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                files.push(path);
            }
        }
    }

    fn relative(path: &Path) -> String {
        path.strip_prefix(repo_root())
            .unwrap_or(path)
            .display()
            .to_string()
    }

    #[test]
    fn runtime_and_examples_do_not_embed_wgsl() {
        let root = repo_root();
        let mut files = Vec::new();
        collect_rs_files(&root.join("src"), &mut files);
        collect_rs_files(&root.join("examples"), &mut files);

        let allowlist = [
            Path::new("src/hot_reload/shader_loader.rs"),
            Path::new("src/architecture_guard.rs"),
        ];
        let mut offenders = Vec::new();

        for file in files {
            let rel = file.strip_prefix(&root).unwrap_or(&file);
            if allowlist.iter().any(|allowed| rel == *allowed) {
                continue;
            }
            let contents = fs::read_to_string(&file)
                .unwrap_or_else(|err| panic!("failed to read {}: {err}", file.display()));
            if contents.contains("ShaderSource::Wgsl")
                || contents.contains("const OVERLAY_SHADER")
                || contents.contains("const PRESENT_SHADER")
                || contents.contains("shader_source = r#\"")
            {
                offenders.push(relative(&file));
            }
        }

        assert!(
            offenders.is_empty(),
            "repo-tracked WGSL is forbidden outside the allowlist: {offenders:?}"
        );
    }

    #[test]
    fn runtime_and_examples_do_not_leave_uniform_min_binding_size_implicit() {
        let root = repo_root();
        let mut files = Vec::new();
        collect_rs_files(&root.join("src"), &mut files);
        collect_rs_files(&root.join("examples"), &mut files);

        let mut offenders = Vec::new();
        for file in files {
            if relative(&file) == "src/architecture_guard.rs" {
                continue;
            }
            let contents = fs::read_to_string(&file)
                .unwrap_or_else(|err| panic!("failed to read {}: {err}", file.display()));
            if contents.contains("min_binding_size: None") {
                offenders.push(relative(&file));
            }
        }

        assert!(
            offenders.is_empty(),
            "uniform/storage bindings should set min_binding_size explicitly: {offenders:?}"
        );
    }

    #[test]
    fn runtime_and_examples_do_not_reintroduce_removed_gpu_abi_mirrors() {
        let root = repo_root();
        let mut files = Vec::new();
        collect_rs_files(&root.join("src"), &mut files);
        collect_rs_files(&root.join("examples"), &mut files);

        let mut offenders = Vec::new();
        for file in files {
            if relative(&file) == "src/architecture_guard.rs" {
                continue;
            }
            let contents = fs::read_to_string(&file)
                .unwrap_or_else(|err| panic!("failed to read {}: {err}", file.display()));
            let has_removed_mirror = contents.lines().any(|line| {
                let trimmed = line.trim_start();
                trimmed.starts_with("struct GpuGridCell")
                    || trimmed.starts_with("struct AtlasGridCell")
                    || trimmed.starts_with("struct GpuBezierCurve")
                    || trimmed.starts_with("struct GpuGlyphData")
                    || trimmed.starts_with("struct GpuCharInstanceEx")
                    || trimmed.starts_with("struct GpuUiPrimitive")
                    || trimmed.starts_with("pub grid_cells:")
                    || trimmed.starts_with("pub curve_indices:")
                    || trimmed.contains("let grid_cells_buffer =")
                    || trimmed.contains("let curve_indices_buffer =")
            });
            if has_removed_mirror {
                offenders.push(relative(&file));
            }
        }

        assert!(
            offenders.is_empty(),
            "removed handwritten GPU ABI mirrors must not reappear: {offenders:?}"
        );
    }

    #[test]
    fn vector_text_shaders_do_not_reintroduce_dead_glyph_grid_abi() {
        let root = repo_root();
        let shaders = [
            "shaders/sdf_text2d_vector.slang",
            "shaders/sdf_todomvc.slang",
            "shaders/sdf_clay_vector.slang",
            "shaders/sdf_text_shadow_vector.slang",
            "shaders/sdf_todomvc_3d.slang",
        ];
        let forbidden = [
            "struct GridCell",
            "StructuredBuffer<GridCell>",
            "StructuredBuffer<uint> curveIndices",
            "uint4 gridInfo",
        ];

        let mut offenders = Vec::new();
        for shader in shaders {
            let path = root.join(shader);
            let contents = fs::read_to_string(&path)
                .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
            if forbidden.iter().any(|pattern| contents.contains(pattern)) {
                offenders.push(shader.to_string());
            }
        }

        assert!(
            offenders.is_empty(),
            "dead glyph-grid shader ABI must not reappear: {offenders:?}"
        );
    }

    #[test]
    fn shared_retained_generated_abi_matches_ui_physical_shader_layouts() {
        type SharedBezierCurve = ui2d_shader_bindings::BezierCurve_std430_0;
        type PhysicalBezierCurve = ui_physical_shader_bindings::BezierCurve_std430_0;
        assert_eq!(
            size_of::<SharedBezierCurve>(),
            size_of::<PhysicalBezierCurve>()
        );
        assert_eq!(
            align_of::<SharedBezierCurve>(),
            align_of::<PhysicalBezierCurve>()
        );
        assert_eq!(
            std::mem::offset_of!(SharedBezierCurve, points01_0),
            std::mem::offset_of!(PhysicalBezierCurve, points01_0)
        );
        assert_eq!(
            std::mem::offset_of!(SharedBezierCurve, points2bbox_0),
            std::mem::offset_of!(PhysicalBezierCurve, points2bbox_0)
        );
        assert_eq!(
            std::mem::offset_of!(SharedBezierCurve, bboxFlags_0),
            std::mem::offset_of!(PhysicalBezierCurve, bboxFlags_0)
        );

        type SharedGlyphData = ui2d_shader_bindings::GlyphData_std430_0;
        type PhysicalGlyphData = ui_physical_shader_bindings::GlyphData_std430_0;
        assert_eq!(size_of::<SharedGlyphData>(), size_of::<PhysicalGlyphData>());
        assert_eq!(
            align_of::<SharedGlyphData>(),
            align_of::<PhysicalGlyphData>()
        );
        assert_eq!(
            std::mem::offset_of!(SharedGlyphData, bounds_0),
            std::mem::offset_of!(PhysicalGlyphData, bounds_0)
        );
        assert_eq!(
            std::mem::offset_of!(SharedGlyphData, curveInfo_0),
            std::mem::offset_of!(PhysicalGlyphData, curveInfo_0)
        );

        type SharedCharInstance = ui2d_shader_bindings::CharInstanceEx_std430_0;
        type PhysicalCharInstance = ui_physical_shader_bindings::CharInstanceEx_std430_0;
        assert_eq!(
            size_of::<SharedCharInstance>(),
            size_of::<PhysicalCharInstance>()
        );
        assert_eq!(
            align_of::<SharedCharInstance>(),
            align_of::<PhysicalCharInstance>()
        );
        assert_eq!(
            std::mem::offset_of!(SharedCharInstance, posAndChar_0),
            std::mem::offset_of!(PhysicalCharInstance, posAndChar_0)
        );
        assert_eq!(
            std::mem::offset_of!(SharedCharInstance, colorFlags_0),
            std::mem::offset_of!(PhysicalCharInstance, colorFlags_0)
        );

        type SharedUiPrimitive = ui2d_shader_bindings::UiPrimitive_std430_0;
        type PhysicalUiPrimitive = ui_physical_shader_bindings::UiPrimitive_std430_0;
        assert_eq!(
            size_of::<SharedUiPrimitive>(),
            size_of::<PhysicalUiPrimitive>()
        );
        assert_eq!(
            align_of::<SharedUiPrimitive>(),
            align_of::<PhysicalUiPrimitive>()
        );
        assert_eq!(
            std::mem::offset_of!(SharedUiPrimitive, posSize_0),
            std::mem::offset_of!(PhysicalUiPrimitive, posSize_0)
        );
        assert_eq!(
            std::mem::offset_of!(SharedUiPrimitive, color_0),
            std::mem::offset_of!(PhysicalUiPrimitive, color_0)
        );
        assert_eq!(
            std::mem::offset_of!(SharedUiPrimitive, params_0),
            std::mem::offset_of!(PhysicalUiPrimitive, params_0)
        );
        assert_eq!(
            std::mem::offset_of!(SharedUiPrimitive, extra_0),
            std::mem::offset_of!(PhysicalUiPrimitive, extra_0)
        );
    }
}
