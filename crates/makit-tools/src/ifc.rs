//! IFC files: extraction to the generic JSON format, on demand.
//!
//! The Python extractor (`analyze_ifc.py`, IfcOpenShell) writes `<name>.json`
//! beside an IFC file. [`extraction_for`] is the one entry point the CLI, the
//! TUI picker and `murb-simulate` use: it finds the extraction, runs the
//! extractor when there is none or it is stale, and never makes a fresh JSON
//! worse than it was.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use makit_core::models::{CancelToken, TaskContext};
use makit_core::registry::Registry;

use crate::building_model;
use crate::python;

/// The extractor script, by its path from the repository root.
pub const EXTRACTOR: &str = "pyrevit-extension/Makit.extension/lib/analyze_ifc.py";

/// How much newer than its JSON an IFC file must be to count as changed. A
/// git checkout writes both within milliseconds, in either order.
pub const STALE_TOLERANCE: Duration = Duration::from_secs(2);

/// Where an IFC file's extraction was found or made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub json: PathBuf,
    /// The extractor ran just now.
    pub extracted: bool,
    /// Why a stale extraction is being used anyway.
    pub note: Option<String>,
}

/// Run the extractor on `ifc`, writing `output` (`<name>.json` beside the
/// IFC by default). The JSON is written to a temporary file and renamed into
/// place, so a reader never sees a partial one. `cancel` stops it early.
pub fn extract(
    ifc: &Path,
    output: Option<&Path>,
    cancel: Option<&CancelToken>,
) -> anyhow::Result<PathBuf> {
    if !ifc.is_file() {
        anyhow::bail!("{} is not a file", ifc.display());
    }
    let output = output
        .map(Path::to_path_buf)
        .unwrap_or_else(|| building_model::extraction_path(ifc));
    let tmp = output.with_extension("json.tmp");
    let python = python::find("ifcopenshell")?;
    let script = python::repo_file(EXTRACTOR)?;
    let cancelled = || cancel.is_some_and(CancelToken::is_cancelled);
    if cancelled() {
        anyhow::bail!("cancelled");
    }

    let mut child = Command::new(&python)
        .arg(&script)
        .arg(ifc)
        .args(["--extract-only", "--output"])
        .arg(&tmp)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("cannot run {}: {e}", python.display()))?;
    // Drain stderr on its own thread so a chatty extractor never blocks.
    let mut stderr = child.stderr.take();
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        if let Some(stderr) = stderr.as_mut() {
            let _ = stderr.read_to_string(&mut text);
        }
        text
    });
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            let _ = reader.join();
            let _ = std::fs::remove_file(&tmp);
            anyhow::bail!("cancelled");
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let stderr = reader.join().unwrap_or_default();
    if !status.success() {
        let _ = std::fs::remove_file(&tmp);
        anyhow::bail!(
            "analyze_ifc.py failed ({status}): {}",
            stderr.trim().lines().last().unwrap_or("no error output")
        );
    }
    if !tmp.is_file() {
        anyhow::bail!("analyze_ifc.py wrote nothing to {}", tmp.display());
    }
    std::fs::rename(&tmp, &output)?;
    Ok(output)
}

/// Whether `json` no longer stands for `ifc`: the IFC is newer by more than
/// [`STALE_TOLERANCE`], or the JSON predates storeys and slabs (format 1).
pub fn is_stale(ifc: &Path, json: &Path) -> bool {
    let version = building_model::load_model(json)
        .map(|model| model.format_version())
        .unwrap_or(1);
    if version < 2 {
        return true;
    }
    let modified = |path: &Path| std::fs::metadata(path).and_then(|m| m.modified());
    match (modified(ifc), modified(json)) {
        (Ok(ifc_time), Ok(json_time)) => ifc_time
            .duration_since(json_time)
            .is_ok_and(|newer_by| newer_by > STALE_TOLERANCE),
        _ => false,
    }
}

/// Whether `ifc` has no extraction beside it, or only a stale one.
pub fn needs_extraction(ifc: &Path) -> bool {
    match building_model::find_json_for_ifc(ifc) {
        Some(json) => is_stale(ifc, &json),
        None => true,
    }
}

/// Decide what to use for `ifc` given what was `found` beside it, whether
/// that is `stale`, and an `extract` step. A fresh extraction is used as is.
/// A stale one is replaced, or kept with a note when extraction fails. With
/// none, extraction must succeed.
pub fn resolve(
    ifc: &Path,
    found: Option<PathBuf>,
    stale: bool,
    extract: impl FnOnce() -> anyhow::Result<PathBuf>,
) -> anyhow::Result<Resolved> {
    let fresh = |json: PathBuf| Resolved {
        json,
        extracted: true,
        note: None,
    };
    match found {
        Some(json) if !stale => Ok(Resolved {
            json,
            extracted: false,
            note: None,
        }),
        Some(json) => match extract() {
            Ok(made) => Ok(fresh(made)),
            Err(err) => Ok(Resolved {
                note: Some(format!("using stale {}: {err:#}", json.display())),
                json,
                extracted: false,
            }),
        },
        None => match extract() {
            Ok(made) => Ok(fresh(made)),
            Err(err) => anyhow::bail!(
                "{} is not extracted and extraction failed: {err:#}",
                ifc.display()
            ),
        },
    }
}

/// The extraction to use for `ifc`, made or refreshed when needed.
pub fn extraction_for(ifc: &Path, cancel: Option<&CancelToken>) -> anyhow::Result<Resolved> {
    let found = building_model::find_json_for_ifc(ifc);
    let stale = found.as_deref().is_some_and(|json| is_stale(ifc, json));
    resolve(ifc, found, stale, || extract(ifc, None, cancel))
}

fn is_true(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}

/// Extract `ifc` and say what happened: to `output` or beside the file,
/// skipping a fresh extraction unless `force`. Shared by the `ifc` source
/// and `makit extract`.
pub fn run_extract(
    ifc: &Path,
    output: Option<&Path>,
    force: bool,
    cancel: Option<&CancelToken>,
) -> anyhow::Result<Resolved> {
    let resolved = if force || output.is_some() {
        Resolved {
            json: extract(ifc, output, cancel)?,
            extracted: true,
            note: None,
        }
    } else {
        extraction_for(ifc, cancel)?
    };
    if resolved.extracted {
        println!("Extracted to {}", resolved.json.display());
    } else {
        println!("Already extracted: {}", resolved.json.display());
    }
    if let Some(note) = &resolved.note {
        println!("  {note}");
    }
    let model = building_model::load_model(&resolved.json)?;
    println!(
        "  {} walls, {} windows, {} slabs, {} storeys",
        model.walls.len(),
        model.windows.len(),
        model.slabs.len(),
        model.storeys.len()
    );
    Ok(resolved)
}

/// The `ifc` source: extract `--file` to JSON (beside it, or to `--output`),
/// even when a fresh one exists with `--force`.
fn handle_extract(ctx: &TaskContext) -> anyhow::Result<()> {
    let file = ctx.get_option("file", "");
    if file.trim().is_empty() {
        anyhow::bail!("--file is required: path to an IFC file");
    }
    let output = ctx.get_option("output", "");
    let output = (!output.trim().is_empty()).then(|| PathBuf::from(output.trim()));
    run_extract(
        Path::new(file.trim()),
        output.as_deref(),
        is_true(&ctx.get_option("force", "")),
        Some(&ctx.cancel),
    )?;
    Ok(())
}

pub fn register_tasks(reg: &mut Registry) {
    reg.register_source(
        "ifc",
        "IFC file: extract it to JSON beside it",
        Arc::new(handle_extract),
    )
    .add_option("file", "Path to IFC file", "string", true, None)
    .add_option(
        "output",
        "Where to write the extraction (default: <name>.json beside the IFC)",
        "string",
        false,
        None,
    )
    .add_option(
        "force",
        "Extract even when a fresh extraction exists",
        "bool",
        false,
        None,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;
    use std::path::PathBuf;

    /// A fresh, empty folder for one test.
    fn folder(test: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("makit-ifc-{}-{test}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn touch(path: &Path) {
        std::fs::write(path, "ISO-10303-21;").unwrap();
    }

    fn write_model(path: &Path, format_version: Option<u32>) {
        let metadata = match format_version {
            Some(v) => serde_json::json!({ "projectName": "p", "formatVersion": v }),
            None => serde_json::json!({ "projectName": "p" }),
        };
        let model =
            serde_json::json!({ "walls": [], "storeys": [], "slabs": [], "metadata": metadata });
        std::fs::write(path, model.to_string()).unwrap();
    }

    fn set_mtime(path: &Path, when: std::time::SystemTime) {
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(when)
            .unwrap();
    }

    #[test]
    fn a_fresh_extraction_is_used_without_extracting() {
        let r = resolve(Path::new("a.ifc"), Some("a.json".into()), false, || {
            panic!("must not extract")
        })
        .unwrap();
        assert_eq!(
            r,
            Resolved {
                json: "a.json".into(),
                extracted: false,
                note: None
            }
        );
    }

    #[test]
    fn a_stale_extraction_is_replaced_or_kept_with_a_note() {
        let r = resolve(Path::new("a.ifc"), Some("a.json".into()), true, || {
            Ok("a.json".into())
        })
        .unwrap();
        assert!(r.extracted && r.note.is_none());

        let r = resolve(Path::new("a.ifc"), Some("a.json".into()), true, || {
            Err(anyhow!("no python"))
        })
        .unwrap();
        assert_eq!(r.json, PathBuf::from("a.json"));
        assert!(!r.extracted);
        let note = r.note.as_deref().unwrap();
        assert!(
            note.contains("stale") && note.contains("no python"),
            "{note}"
        );
    }

    #[test]
    fn a_missing_extraction_that_cannot_be_made_is_an_error() {
        let err = resolve(Path::new("a.ifc"), None, false, || {
            Err(anyhow!("no python"))
        })
        .unwrap_err()
        .to_string();
        assert!(err.contains("a.ifc") && err.contains("no python"), "{err}");
        let r = resolve(Path::new("a.ifc"), None, false, || Ok("a.json".into())).unwrap();
        assert!(r.extracted);
    }

    #[test]
    fn staleness_tolerates_a_checkout_and_old_formats() {
        let dir = folder("stale");
        let ifc = dir.join("m.ifc");
        let json = dir.join("m.json");
        write_model(&json, Some(2));
        touch(&ifc); // written moments after the JSON, like a checkout
        assert!(!is_stale(&ifc, &json));

        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
        set_mtime(&ifc, later);
        assert!(is_stale(&ifc, &json));

        let old = dir.join("old.json");
        write_model(&old, None); // a format-1 extraction
        set_mtime(
            &ifc,
            std::time::SystemTime::now() - std::time::Duration::from_secs(60),
        );
        assert!(is_stale(&ifc, &old));
        assert!(!is_stale(&ifc, &json));
    }

    #[test]
    fn a_bare_filename_resolves_beside_itself() {
        assert_eq!(
            building_model::extraction_path(Path::new("lonely.ifc")),
            PathBuf::from("lonely.json")
        );
        let dir = folder("bare");
        let ifc = dir.join("lonely.ifc");
        touch(&ifc);
        assert!(needs_extraction(&ifc));
        write_model(&dir.join("lonely.json"), Some(2)); // written after the ifc, so newer
        assert!(!needs_extraction(&ifc));
        let r = extraction_for(&ifc, None).unwrap(); // must not look for Python
        assert_eq!(
            r,
            Resolved {
                json: dir.join("lonely.json"),
                extracted: false,
                note: None
            }
        );
    }

    #[test]
    fn a_failed_extraction_leaves_no_temp_file() {
        // Whether or not a Python with IfcOpenShell exists, extracting a
        // file that is not there fails and leaves nothing behind.
        let dir = folder("tmpfile");
        let ifc = dir.join("missing.ifc");
        assert!(extract(&ifc, None, None).is_err());
        assert!(!dir.join("missing.json.tmp").exists());
        assert!(!dir.join("missing.json").exists());
    }

    #[test]
    fn a_cancelled_extraction_stops_before_running() {
        let dir = folder("cancel");
        let ifc = dir.join("m.ifc");
        touch(&ifc);
        let cancel = CancelToken::new();
        cancel.cancel();
        let err = extract(&ifc, None, Some(&cancel)).unwrap_err().to_string();
        assert!(err.contains("cancelled"), "{err}");
        assert!(!dir.join("m.json").exists());
    }

    #[test]
    fn the_ifc_source_registers_its_options() {
        let mut reg = Registry::default();
        register_tasks(&mut reg);
        let names: Vec<(&str, bool)> = reg.sources["ifc"]
            .options
            .iter()
            .map(|o| (o.name.as_str(), o.required))
            .collect();
        assert_eq!(
            names,
            vec![("file", true), ("output", false), ("force", false)]
        );
    }
}
