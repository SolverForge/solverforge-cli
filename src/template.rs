use include_dir::{Dir, DirEntry};
use std::fs;
use std::path::Path;

/// Renders a template directory into `dest`, replacing `{{key}}` placeholders.
pub fn render(dir: &Dir, dest: &Path, vars: &[(&str, &str)]) -> Result<(), String> {
    render_dir(dir, dest, dest, vars)
}

fn render_dir(
    dir: &Dir,
    dest_root: &Path,
    dest: &Path,
    vars: &[(&str, &str)],
) -> Result<(), String> {
    fs::create_dir_all(dest)
        .map_err(|e| format!("failed to create directory {:?}: {}", dest, e))?;

    for entry in dir.entries() {
        match entry {
            DirEntry::Dir(sub) => {
                let sub_name = sub
                    .path()
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string();
                let sub_dest = dest.join(&sub_name);
                render_dir(sub, dest_root, &sub_dest, vars)?;
            }
            DirEntry::File(file) => {
                let file_name_raw = file
                    .path()
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string();

                // Strip .tmpl extension if present
                let file_name = if file_name_raw.ends_with(".tmpl") {
                    file_name_raw[..file_name_raw.len() - 5].to_string()
                } else {
                    file_name_raw
                };

                let out_path = dest.join(&file_name);

                let contents = file.contents();

                // Binary files (non-UTF-8): copy as-is
                let Ok(text) = std::str::from_utf8(contents) else {
                    fs::write(&out_path, contents)
                        .map_err(|e| format!("failed to write {:?}: {}", out_path, e))?;
                    continue;
                };

                // Text files: apply placeholder substitution
                let rendered = apply_vars(text, vars);
                fs::write(&out_path, rendered)
                    .map_err(|e| format!("failed to write {:?}: {}", out_path, e))?;
            }
        }
    }

    Ok(())
}

fn apply_vars(text: &str, vars: &[(&str, &str)]) -> String {
    let mut result = text.to_string();
    for (key, value) in vars {
        let placeholder = format!("{{{{{}}}}}", key);
        result = result.replace(&placeholder, value);
    }
    result
}
