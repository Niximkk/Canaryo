use std::{
    fs, io,
    path::{Path, PathBuf},
};

pub fn resolve(parent_file: &str, specifier: &str) -> io::Result<PathBuf> {
    let parent = Path::new(parent_file)
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let requested = Path::new(specifier);

    if requested.is_absolute() {
        return resolve_candidate(requested);
    }

    if specifier.starts_with("./") || specifier.starts_with("../") {
        return resolve_candidate(&parent.join(requested));
    }

    let (package, subpath) = split_package_specifier(specifier);
    for directory in parent.ancestors() {
        let mut candidate = directory.join("node_modules").join(package);
        if !subpath.is_empty() {
            candidate = candidate.join(subpath);
        }
        if let Ok(resolved) = resolve_candidate(&candidate) {
            return Ok(resolved);
        }
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("módulo '{specifier}' não encontrado a partir de {parent_file}"),
    ))
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
        return candidate.canonicalize();
    }

    for extension in [".js", ".json", ".cjs"] {
        let mut filename = candidate.as_os_str().to_os_string();
        filename.push(extension);
        let file = PathBuf::from(filename);
        if file.is_file() {
            return file.canonicalize();
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

        for index in ["index.js", "index.json", "index.cjs"] {
            let file = candidate.join(index);
            if file.is_file() {
                return file.canonicalize();
            }
        }
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("{} não existe", candidate.display()),
    ))
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

        assert_eq!(resolved, module.canonicalize().unwrap());
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

        assert_eq!(
            resolved,
            package.join("lib/main.js").canonicalize().unwrap()
        );
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
