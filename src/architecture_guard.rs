#[cfg(test)]
mod tests {
    use std::fs;
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
}
