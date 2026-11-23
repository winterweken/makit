//! Finding a Python for the bridges: the IFC extractor needs IfcOpenShell,
//! the MURB bridge needs murb_energy_tool.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// How to get a Python the IFC extractor can use, for error messages.
pub const SETUP_HINT: &str = "set up a Python with IfcOpenShell once: \
    uv venv .venv --python 3.13 && uv pip install --python .venv/bin/python ifcopenshell \
    (or point MAKIT_PYTHON at one)";

/// How to get a Python that has `module`, for error messages: the IfcOpenShell
/// setup for the extractor; otherwise the package goes on `PYTHONPATH`.
pub fn setup_hint(module: &str) -> String {
    if module == "ifcopenshell" {
        return SETUP_HINT.to_string();
    }
    format!(
        "put the package that provides {module} on PYTHONPATH, \
         or point MAKIT_PYTHON at a Python that has it"
    )
}

/// An interpreter worth trying, and whether the user named it (`MAKIT_PYTHON`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub path: PathBuf,
    pub explicit: bool,
}

/// Interpreters to try, in order: `$MAKIT_PYTHON`, the repository's `.venv`,
/// then `python3` and `python` on the PATH.
pub fn candidates() -> Vec<Candidate> {
    let explicit = std::env::var_os("MAKIT_PYTHON").map(PathBuf::from);
    let root = repo_root().ok();
    candidates_from(explicit, root.as_deref())
}

/// [`candidates`] for a given `MAKIT_PYTHON` value and repository root.
pub fn candidates_from(explicit: Option<PathBuf>, root: Option<&Path>) -> Vec<Candidate> {
    let mut out = Vec::new();
    if let Some(path) = explicit.filter(|p| !p.as_os_str().is_empty()) {
        out.push(Candidate {
            path,
            explicit: true,
        });
    }
    if let Some(root) = root {
        let venv = root.join(".venv").join(if cfg!(windows) {
            "Scripts/python.exe"
        } else {
            "bin/python"
        });
        if venv.exists() {
            out.push(Candidate {
                path: venv,
                explicit: false,
            });
        }
    }
    for name in ["python3", "python"] {
        out.push(Candidate {
            path: PathBuf::from(name),
            explicit: false,
        });
    }
    out
}

/// Whether `python -c "import <module>"` succeeds.
pub fn imports(python: &Path, module: &str) -> bool {
    Command::new(python)
        .args(["-c", &format!("import {module}")])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// The first interpreter among [`candidates`] that imports `module`.
pub fn find(module: &str) -> anyhow::Result<PathBuf> {
    find_with(candidates(), module, &imports)
}

/// [`find`] over given candidates with a given probe. An explicit candidate
/// that fails the probe is the error: the user named it, so nothing else is
/// tried behind their back.
pub fn find_with(
    candidates: Vec<Candidate>,
    module: &str,
    probe: &dyn Fn(&Path, &str) -> bool,
) -> anyhow::Result<PathBuf> {
    let mut tried = Vec::new();
    for candidate in candidates {
        if probe(&candidate.path, module) {
            return Ok(candidate.path);
        }
        if candidate.explicit {
            anyhow::bail!(
                "MAKIT_PYTHON={} cannot import {module}",
                candidate.path.display()
            );
        }
        tried.push(candidate.path.display().to_string());
    }
    anyhow::bail!(
        "no Python with {module} found (tried {}); {}",
        tried.join(", "),
        setup_hint(module)
    )
}

/// The makit repository root: the nearest ancestor of the executable's
/// folder, then of the working directory, that holds `scripts/murb_runner.py`.
pub fn repo_root() -> anyhow::Result<PathBuf> {
    let mut starts = Vec::new();
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        starts.push(dir);
    }
    if let Ok(cwd) = std::env::current_dir() {
        starts.push(cwd);
    }
    for start in &starts {
        for dir in start.ancestors() {
            if dir.join("scripts").join("murb_runner.py").is_file() {
                return Ok(dir.to_path_buf());
            }
        }
    }
    anyhow::bail!(
        "makit repository not found from {}: run makit from its checkout",
        starts
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(" or ")
    )
}

/// A file of the makit repository by its path from the root.
pub fn repo_file(relative: &str) -> anyhow::Result<PathBuf> {
    let path = repo_root()?.join(relative);
    if !path.exists() {
        anyhow::bail!("{} is missing from the makit repository", path.display());
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(path: &str, explicit: bool) -> Candidate {
        Candidate {
            path: PathBuf::from(path),
            explicit,
        }
    }

    #[test]
    fn an_explicit_python_without_the_module_is_an_error_not_a_fallback() {
        let cands = vec![cand("/x/py", true), cand("/usr/bin/python3", false)];
        let err = find_with(cands, "ifcopenshell", &|p, _| p.ends_with("python3"))
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("MAKIT_PYTHON=/x/py cannot import ifcopenshell"),
            "{err}"
        );
    }

    #[test]
    fn the_first_candidate_that_imports_the_module_wins() {
        let cands = vec![cand(".venv/bin/python", false), cand("python3", false)];
        assert_eq!(
            find_with(cands, "ifcopenshell", &|p, _| p.ends_with("python3")).unwrap(),
            PathBuf::from("python3")
        );
    }

    #[test]
    fn no_candidate_names_what_was_tried_and_how_to_set_up() {
        let err = find_with(vec![cand("python3", false)], "ifcopenshell", &|_, _| false)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("python3") && err.contains("ifcopenshell") && err.contains("uv venv"),
            "{err}"
        );
    }

    #[test]
    fn the_hint_names_the_package_that_is_missing() {
        // murb_energy_tool does not come with IfcOpenShell: its hint is PYTHONPATH
        let err = find_with(vec![cand("python3", false)], "murb_energy_tool", &|_, _| {
            false
        })
        .unwrap_err()
        .to_string();
        assert!(
            err.contains("PYTHONPATH") && err.contains("murb_energy_tool"),
            "{err}"
        );
        assert!(!err.contains("IfcOpenShell"), "{err}");
    }

    #[test]
    fn candidates_prefer_makit_python_then_the_repo_venv_then_path() {
        let dir = std::env::temp_dir().join(format!("makit-python-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let bin = dir
            .join(".venv")
            .join(if cfg!(windows) { "Scripts" } else { "bin" });
        std::fs::create_dir_all(&bin).unwrap();
        let exe = bin.join(if cfg!(windows) {
            "python.exe"
        } else {
            "python"
        });
        std::fs::write(&exe, "").unwrap();

        let cands = candidates_from(Some(PathBuf::from("/opt/py")), Some(&dir));
        let paths: Vec<(PathBuf, bool)> = cands.into_iter().map(|c| (c.path, c.explicit)).collect();
        assert_eq!(
            paths,
            vec![
                (PathBuf::from("/opt/py"), true),
                (exe.clone(), false),
                (PathBuf::from("python3"), false),
                (PathBuf::from("python"), false),
            ]
        );

        // Without a venv in the repository, only the PATH names remain.
        std::fs::remove_file(&exe).unwrap();
        let names: Vec<PathBuf> = candidates_from(None, Some(&dir))
            .into_iter()
            .map(|c| c.path)
            .collect();
        assert_eq!(
            names,
            vec![PathBuf::from("python3"), PathBuf::from("python")]
        );
    }

    #[test]
    fn imports_probes_a_real_interpreter() {
        let python = PathBuf::from("python3");
        if !imports(&python, "sys") {
            eprintln!("no python3 on PATH; skipping");
            return;
        }
        assert!(!imports(&python, "makit_module_that_does_not_exist"));
    }

    #[test]
    fn repo_file_finds_the_runner_from_the_test_cwd() {
        assert!(repo_file("scripts/murb_runner.py")
            .unwrap()
            .ends_with("scripts/murb_runner.py"));
        assert!(
            repo_file("pyrevit-extension/Makit.extension/lib/analyze_ifc.py")
                .unwrap()
                .exists()
        );
        assert!(repo_file("scripts/not_here.py").is_err());
    }
}
