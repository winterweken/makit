//! `makit extract` — extract an IFC file to the generic JSON format.

use std::path::Path;

use anyhow::Result;
use makit_tools::ifc;

/// Extract `file` to `output` (default: `<name>.json` beside it); a fresh
/// extraction is kept unless `force`.
pub fn run(file: &str, output: Option<&str>, force: bool) -> Result<()> {
    ifc::run_extract(Path::new(file), output.map(Path::new), force, None)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_file_is_an_error_before_python_is_looked_for() {
        let err = run("/nowhere/missing.ifc", None, false)
            .unwrap_err()
            .to_string();
        assert!(err.contains("is not a file"), "{err}");
    }
}
