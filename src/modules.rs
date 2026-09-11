use std::{
    fs, io,
    path::{Component, Path, PathBuf},
};

pub fn resolve(parent_file: &str, specifier: &str) -> io::Result<PathBuf> {
    resolve_with_conditions(parent_file, specifier, &["require", "node", "default"])
}

pub fn resolve_import(parent_file: &str, specifier: &str) -> io::Result<PathBuf> {
    resolve_with_conditions(parent_file, specifier, &["import", "node", "default"])
}

pub fn is_esm_path(path: &Path) -> bool {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("mjs") => return true,
        Some("cjs") => return false,
        Some("js") => {}
        _ => return false,
    }

    for directory in path.parent().into_iter().flat_map(Path::ancestors) {
        let package_file = directory.join("package.json");
        if !package_file.is_file() {
            continue;
        }
        return fs::read_to_string(package_file)
            .ok()
            .and_then(|contents| serde_json::from_str::<serde_json::Value>(&contents).ok())
            .and_then(|package| {
                package
                    .get("type")
                    .and_then(|value| value.as_str())
                    .map(str::to_owned)
            })
            .is_some_and(|kind| kind == "module");
    }
    false
}

fn resolve_with_conditions(
    parent_file: &str,
    specifier: &str,
    conditions: &[&str],
) -> io::Result<PathBuf> {
    let parent = Path::new(parent_file)
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let requested = Path::new(specifier);

    if requested.is_absolute() {
        return resolve_candidate(requested);
    }

    if matches!(specifier, "." | "..")
        || specifier.starts_with("./")
        || specifier.starts_with("../")
    {
        return resolve_candidate(&parent.join(requested));
    }

    let (package, subpath) = split_package_specifier(specifier);
    for directory in parent.ancestors() {
        let package_directory = directory.join("node_modules").join(package);
        if package_directory.is_dir() {
            return resolve_package(&package_directory, subpath, conditions);
        }
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("módulo '{specifier}' não encontrado a partir de {parent_file}"),
    ))
}

fn resolve_package(
    package_directory: &Path,
    subpath: &str,
    conditions: &[&str],
) -> io::Result<PathBuf> {
    let package_file = package_directory.join("package.json");
    if package_file.is_file()
        && let Ok(contents) = fs::read_to_string(&package_file)
        && let Ok(package) = serde_json::from_str::<serde_json::Value>(&contents)
        && let Some(exports) = package.get("exports")
    {
        let export_key = if subpath.is_empty() {
            ".".to_string()
        } else {
            format!("./{subpath}")
        };
        let target = resolve_exports(exports, &export_key, conditions).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "subpath '{export_key}' não exportado por {}",
                    package_file.display()
                ),
            )
        })?;
        if !target.starts_with("./") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("destino de exports inválido: '{target}'"),
            ));
        }
        let candidate = normalize(&package_directory.join(&target[2..]));
        let package_root = normalize(package_directory);
        if !candidate.starts_with(&package_root) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("destino de exports escapa do pacote: '{target}'"),
            ));
        }
        return resolve_candidate(&candidate);
    }

    let candidate = if subpath.is_empty() {
        package_directory.to_path_buf()
    } else {
        package_directory.join(subpath)
    };
    resolve_candidate(&candidate)
}

fn resolve_exports(exports: &serde_json::Value, key: &str, conditions: &[&str]) -> Option<String> {
    match exports {
        serde_json::Value::String(target) if key == "." => Some(target.clone()),
        serde_json::Value::Array(targets) => targets
            .iter()
            .find_map(|target| resolve_exports(target, key, conditions)),
        serde_json::Value::Object(entries) => {
            let uses_subpaths = entries.keys().any(|entry| entry.starts_with('.'));
            if uses_subpaths {
                if let Some(target) = entries.get(key) {
                    return resolve_export_target(target, None, conditions);
                }

                let mut patterns = entries
                    .iter()
                    .filter_map(|(pattern, target)| {
                        pattern.contains('*').then_some((pattern, target))
                    })
                    .collect::<Vec<_>>();
                patterns.sort_by_key(|(pattern, _)| std::cmp::Reverse(pattern.len()));
                for (pattern, target) in patterns {
                    let (prefix, suffix) = pattern.split_once('*')?;
                    if key.starts_with(prefix) && key.ends_with(suffix) {
                        let matched = &key[prefix.len()..key.len() - suffix.len()];
                        if let Some(target) =
                            resolve_export_target(target, Some(matched), conditions)
                        {
                            return Some(target);
                        }
                    }
                }
                None
            } else if key == "." {
                resolve_export_target(exports, None, conditions)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn resolve_export_target(
    target: &serde_json::Value,
    replacement: Option<&str>,
    conditions: &[&str],
) -> Option<String> {
    match target {
        serde_json::Value::String(target) => Some(match replacement {
            Some(replacement) => target.replace('*', replacement),
            None => target.clone(),
        }),
        serde_json::Value::Array(targets) => targets
            .iter()
            .find_map(|target| resolve_export_target(target, replacement, conditions)),
        serde_json::Value::Object(targets) => conditions.iter().find_map(|condition| {
            targets
                .get(*condition)
                .and_then(|target| resolve_export_target(target, replacement, conditions))
        }),
        _ => None,
    }
}

fn split_package_specifier(specifier: &str) -> (&str, &str) {
    if specifier.starts_with('@') {
        let second_slash = specifier.match_indices('/').nth(1).map(|(index, _)| index);
        return match second_slash {
            Some(index) => (&specifier[..index], &specifier[index + 1..]),
            None => (specifier, ""),
        };
    }

    match specifier.split_once('/') {
        Some((package, subpath)) => (package, subpath),
        None => (specifier, ""),
    }
}

fn resolve_candidate(candidate: &Path) -> io::Result<PathBuf> {
    if candidate.is_file() {
        return Ok(normalize(candidate));
    }

    for extension in [".js", ".json", ".cjs", ".mjs"] {
        let mut filename = candidate.as_os_str().to_os_string();
        filename.push(extension);
        let file = PathBuf::from(filename);
        if file.is_file() {
            return Ok(normalize(&file));
        }
    }

    if candidate.is_dir() {
        let package_file = candidate.join("package.json");
        if package_file.is_file()
            && let Ok(contents) = fs::read_to_string(&package_file)
            && let Ok(package) = serde_json::from_str::<serde_json::Value>(&contents)
            && let Some(main) = package.get("main").and_then(serde_json::Value::as_str)
        {
            let main_candidate = candidate.join(main);
            if main_candidate != candidate
                && let Ok(resolved) = resolve_candidate(&main_candidate)
            {
                return Ok(resolved);
            }
        }

        for index in ["index.js", "index.json", "index.cjs", "index.mjs"] {
            let file = candidate.join(index);
            if file.is_file() {
                return Ok(normalize(&file));
            }
        }
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("{} não existe", candidate.display()),
    ))
}

fn normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            component => normalized.push(component.as_os_str()),
        }
    }

    normalized
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture() -> PathBuf {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("canaryo-modules-{id}"));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn resolves_relative_javascript_without_extension() {
        let root = fixture();
        let entry = root.join("server.js");
        let module = root.join("routes.js");
        fs::write(&entry, "").unwrap();
        fs::write(&module, "").unwrap();

        let resolved = resolve(entry.to_str().unwrap(), "./routes").unwrap();

        assert_eq!(resolved, normalize(&module));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resolves_an_exact_parent_directory_specifier() {
        let root = fixture();
        let entry = root.join("nested/module.js");
        fs::create_dir_all(entry.parent().unwrap()).unwrap();
        fs::write(&entry, "").unwrap();
        fs::write(root.join("index.js"), "").unwrap();

        let resolved = resolve(entry.to_str().unwrap(), "..").unwrap();

        assert_eq!(resolved, normalize(&root.join("index.js")));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resolves_a_package_main_file() {
        let root = fixture();
        let entry = root.join("server.js");
        let package = root.join("node_modules/example");
        fs::create_dir_all(package.join("lib")).unwrap();
        fs::write(&entry, "").unwrap();
        fs::write(package.join("package.json"), r#"{"main":"lib/main.js"}"#).unwrap();
        fs::write(package.join("lib/main.js"), "").unwrap();

        let resolved = resolve(entry.to_str().unwrap(), "example").unwrap();

        assert_eq!(resolved, normalize(&package.join("lib/main.js")));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resolves_package_exports_and_require_conditions() {
        let root = fixture();
        let entry = root.join("server.js");
        let package = root.join("node_modules/example");
        fs::create_dir_all(package.join("lib")).unwrap();
        fs::write(&entry, "").unwrap();
        fs::write(
            package.join("package.json"),
            r#"{"exports":{".":{"import":"./esm.js","require":"./cjs.js"},"./feature":"./lib/feature.js"}}"#,
        )
        .unwrap();
        fs::write(package.join("cjs.js"), "").unwrap();
        fs::write(package.join("esm.js"), "").unwrap();
        fs::write(package.join("lib/feature.js"), "").unwrap();

        let package_root = resolve(entry.to_str().unwrap(), "example").unwrap();
        let feature = resolve(entry.to_str().unwrap(), "example/feature").unwrap();

        assert_eq!(package_root, normalize(&package.join("cjs.js")));
        assert_eq!(
            resolve_import(entry.to_str().unwrap(), "example").unwrap(),
            normalize(&package.join("esm.js"))
        );
        assert_eq!(feature, normalize(&package.join("lib/feature.js")));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resolves_wildcard_package_exports() {
        let root = fixture();
        let entry = root.join("server.js");
        let package = root.join("node_modules/example");
        fs::create_dir_all(package.join("lib/features")).unwrap();
        fs::write(&entry, "").unwrap();
        fs::write(
            package.join("package.json"),
            r#"{"exports":{"./features/*":"./lib/features/*.js"}}"#,
        )
        .unwrap();
        fs::write(package.join("lib/features/one.js"), "").unwrap();

        let resolved = resolve(entry.to_str().unwrap(), "example/features/one").unwrap();

        assert_eq!(resolved, normalize(&package.join("lib/features/one.js")));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_package_subpaths_hidden_by_exports() {
        let root = fixture();
        let entry = root.join("server.js");
        let package = root.join("node_modules/example");
        fs::create_dir_all(&package).unwrap();
        fs::write(&entry, "").unwrap();
        fs::write(package.join("package.json"), r#"{"exports":"./index.js"}"#).unwrap();
        fs::write(package.join("index.js"), "").unwrap();
        fs::write(package.join("private.js"), "").unwrap();

        let error = resolve(entry.to_str().unwrap(), "example/private").unwrap_err();

        assert!(error.to_string().contains("não exportado"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn detects_mjs_and_module_package_scopes() {
        let root = fixture();
        let module_directory = root.join("module-package");
        fs::create_dir_all(&module_directory).unwrap();
        fs::write(
            module_directory.join("package.json"),
            r#"{"type":"module"}"#,
        )
        .unwrap();
        fs::write(module_directory.join("index.js"), "").unwrap();

        assert!(is_esm_path(&root.join("entry.mjs")));
        assert!(!is_esm_path(&root.join("entry.cjs")));
        assert!(is_esm_path(&module_directory.join("index.js")));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn normalizes_parent_segments_without_filesystem_canonicalization() {
        let root = fixture();
        let path = root.join("routes").join("..").join("server.js");

        assert_eq!(normalize(&path), root.join("server.js"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn splits_scoped_package_subpaths() {
        assert_eq!(
            split_package_specifier("@scope/pkg/lib"),
            ("@scope/pkg", "lib")
        );
    }
}
