use super::defaults;
use super::types::Template;
use once_cell::sync::Lazy;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use tracing::{debug, info, warn};

// Global storage for the bundled templates directory path
static BUNDLED_TEMPLATES_DIR: Lazy<RwLock<Option<PathBuf>>> = Lazy::new(|| RwLock::new(None));

/// Set the bundled templates directory path (called once at app startup)
pub fn set_bundled_templates_dir(path: PathBuf) {
    info!("Bundled templates directory set to: {:?}", path);
    if let Ok(mut dir) = BUNDLED_TEMPLATES_DIR.write() {
        *dir = Some(path);
    }
}

/// Get the user's custom templates directory path
///
/// Returns the platform-specific application data directory for custom templates:
/// - macOS: ~/Library/Application Support/Briefli/templates/
/// - Windows: %APPDATA%\Briefli\templates\
/// - Linux: ~/.config/Briefli/templates/
fn get_custom_templates_dir() -> Option<PathBuf> {
    let mut path = dirs::data_dir()?;
    path.push("Briefli");
    path.push("templates");
    Some(path)
}

/// Load a template from the bundled resources directory
///
/// # Arguments
/// * `template_id` - Template identifier (without .json extension)
///
/// # Returns
/// The template JSON content if found, None otherwise
fn load_bundled_template(template_id: &str) -> Option<String> {
    let bundled_dir = BUNDLED_TEMPLATES_DIR.read().ok()?.clone()?;
    let template_path = bundled_dir.join(format!("{}.json", template_id));

    debug!("Checking for bundled template at: {:?}", template_path);

    match std::fs::read_to_string(&template_path) {
        Ok(content) => {
            info!(
                "Loaded bundled template '{}' from {:?}",
                template_id, template_path
            );
            Some(content)
        }
        Err(e) => {
            debug!("No bundled template '{}' found: {}", template_id, e);
            None
        }
    }
}

/// Load a template from the user's custom templates directory
///
/// # Arguments
/// * `template_id` - Template identifier (without .json extension)
///
/// # Returns
/// The template JSON content if found, None otherwise
fn load_custom_template(template_id: &str) -> Option<String> {
    let custom_dir = get_custom_templates_dir()?;
    let template_path = custom_dir.join(format!("{}.json", template_id));

    debug!("Checking for custom template at: {:?}", template_path);

    match std::fs::read_to_string(&template_path) {
        Ok(content) => {
            info!(
                "Loaded custom template '{}' from {:?}",
                template_id, template_path
            );
            Some(content)
        }
        Err(e) => {
            debug!("No custom template '{}' found: {}", template_id, e);
            None
        }
    }
}

/// Load and parse a template by identifier
///
/// This function implements a fallback strategy:
/// 1. Check user's custom templates directory
/// 2. Check bundled resources directory (app templates)
/// 3. Fall back to built-in embedded templates
/// 4. Return error if not found in any location
///
/// # Arguments
/// * `template_id` - Template identifier (e.g., "daily_standup", "standard_meeting")
///
/// # Returns
/// Parsed and validated Template struct
pub fn get_template(template_id: &str) -> Result<Template, String> {
    info!("Loading template: {}", template_id);

    // Try custom template first, then bundled, then built-in
    let json_content = if let Some(custom_content) = load_custom_template(template_id) {
        debug!("Using custom template for '{}'", template_id);
        custom_content
    } else if let Some(bundled_content) = load_bundled_template(template_id) {
        debug!("Using bundled template for '{}'", template_id);
        bundled_content
    } else if let Some(builtin_content) = defaults::get_builtin_template(template_id) {
        debug!("Using built-in template for '{}'", template_id);
        builtin_content.to_string()
    } else {
        return Err(format!(
            "Template '{}' not found. Available templates: {}",
            template_id,
            list_template_ids().join(", ")
        ));
    };

    // Parse and validate
    validate_and_parse_template(&json_content)
}

/// Validate and parse template JSON
///
/// # Arguments
/// * `json_content` - Raw JSON string
///
/// # Returns
/// Parsed and validated Template struct
pub fn validate_and_parse_template(json_content: &str) -> Result<Template, String> {
    let template: Template = serde_json::from_str(json_content)
        .map_err(|e| format!("Failed to parse template JSON: {}", e))?;

    template.validate()?;

    Ok(template)
}

/// List all available template identifiers
///
/// Returns a combined list of:
/// - Built-in template IDs
/// - Bundled template IDs (from app resources)
/// - Custom template IDs (from user's data directory)
pub fn list_template_ids() -> Vec<String> {
    let mut ids: Vec<String> = defaults::list_builtin_template_ids()
        .into_iter()
        .map(|s| s.to_string())
        .collect();

    // Add bundled templates if directory is set
    if let Ok(bundled_dir_lock) = BUNDLED_TEMPLATES_DIR.read() {
        if let Some(bundled_dir) = bundled_dir_lock.as_ref() {
            if bundled_dir.exists() {
                match std::fs::read_dir(bundled_dir) {
                    Ok(entries) => {
                        for entry in entries.flatten() {
                            if let Some(filename) = entry.file_name().to_str() {
                                if filename.ends_with(".json") {
                                    let id = filename.trim_end_matches(".json").to_string();
                                    if !ids.contains(&id) {
                                        ids.push(id);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to read bundled templates directory: {}", e);
                    }
                }
            }
        }
    }

    // Add custom templates if directory exists
    if let Some(custom_dir) = get_custom_templates_dir() {
        if custom_dir.exists() {
            match std::fs::read_dir(&custom_dir) {
                Ok(entries) => {
                    for entry in entries.flatten() {
                        if let Some(filename) = entry.file_name().to_str() {
                            if filename.ends_with(".json") {
                                let id = filename.trim_end_matches(".json").to_string();
                                if !ids.contains(&id) {
                                    ids.push(id);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to read custom templates directory: {}", e);
                }
            }
        }
    }

    ids.sort();
    ids
}

/// List all available templates with their metadata
///
/// Returns a list of (id, name, description) tuples
pub fn list_templates() -> Vec<(String, String, String)> {
    let mut templates = Vec::new();

    for id in list_template_ids() {
        match get_template(&id) {
            Ok(template) => {
                templates.push((id, template.name, template.description));
            }
            Err(e) => {
                warn!("Failed to load template '{}': {}", id, e);
            }
        }
    }

    templates
}

/// Returns true if `id` is a safe template identifier.
///
/// Restricted to lowercase ASCII letters, digits, '-' and '_' so it maps to a
/// safe filename and cannot be used for path traversal (e.g. `../evil`).
pub fn is_valid_template_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

fn has_custom_template(id: &str) -> bool {
    get_custom_templates_dir()
        .map(|d| d.join(format!("{}.json", id)).is_file())
        .unwrap_or(false)
}

fn has_bundled_template(id: &str) -> bool {
    if let Ok(lock) = BUNDLED_TEMPLATES_DIR.read() {
        if let Some(dir) = lock.as_ref() {
            return dir.join(format!("{}.json", id)).is_file();
        }
    }
    false
}

/// Determine where the *effective* template for `id` is resolved from.
///
/// Mirrors the [`get_template`] fallback order:
/// `"custom"` (user override) > `"bundled"` (app resources) > `"built_in"`
/// (embedded). Returns `"unknown"` when no template with that id exists.
pub fn template_source(id: &str) -> &'static str {
    if has_custom_template(id) {
        "custom"
    } else if has_bundled_template(id) {
        "bundled"
    } else if defaults::get_builtin_template(id).is_some() {
        "built_in"
    } else {
        "unknown"
    }
}

fn save_custom_template_to_dir(dir: &Path, id: &str, json_content: &str) -> Result<(), String> {
    if !is_valid_template_id(id) {
        return Err(format!(
            "Invalid template id '{}'. Use lowercase letters, digits, '-' or '_' (max 64 chars).",
            id
        ));
    }

    // Reject anything that is not a structurally valid template before writing.
    validate_and_parse_template(json_content)?;

    std::fs::create_dir_all(dir)
        .map_err(|e| format!("Failed to create templates directory: {}", e))?;

    let path = dir.join(format!("{}.json", id));
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("Failed to prepare template save: {}", e))?
        .as_nanos();
    let temporary_path = dir.join(format!(".{}.{}.tmp", id, nonce));
    let backup_path = dir.join(format!(".{}.{}.bak", id, nonce));

    let write_result = (|| -> Result<(), String> {
        let mut temporary_file = std::fs::File::create(&temporary_path)
            .map_err(|e| format!("Failed to create temporary template file: {}", e))?;
        temporary_file
            .write_all(json_content.as_bytes())
            .map_err(|e| format!("Failed to write temporary template file: {}", e))?;
        temporary_file
            .sync_all()
            .map_err(|e| format!("Failed to flush temporary template file: {}", e))?;
        drop(temporary_file);

        if path.exists() {
            std::fs::rename(&path, &backup_path)
                .map_err(|e| format!("Failed to prepare existing template for replacement: {}", e))?;
        }

        if let Err(error) = std::fs::rename(&temporary_path, &path) {
            if backup_path.exists() {
                let _ = std::fs::rename(&backup_path, &path);
            }
            return Err(format!("Failed to replace template file: {}", error));
        }

        if backup_path.exists() {
            if let Err(error) = std::fs::remove_file(&backup_path) {
                warn!(
                    "Template '{}' saved, but backup {:?} could not be removed: {}",
                    id, backup_path, error
                );
            }
        }

        Ok(())
    })();

    if write_result.is_err() {
        let _ = std::fs::remove_file(&temporary_path);
    }
    write_result?;

    info!("Saved custom template '{}' to {:?}", id, path);
    Ok(())
}

/// Save (create or overwrite) a custom template in the user's data directory.
///
/// Saving with the same id as a built-in/bundled template creates an override
/// that [`get_template`] will prefer; deleting the override reverts to the
/// original. The JSON is validated before it is written.
pub fn save_custom_template(id: &str, json_content: &str) -> Result<(), String> {
    let dir = get_custom_templates_dir()
        .ok_or_else(|| "Could not resolve custom templates directory".to_string())?;
    save_custom_template_to_dir(&dir, id, json_content)
}

fn delete_custom_template_in_dir(dir: &Path, id: &str) -> Result<(), String> {
    if !is_valid_template_id(id) {
        return Err(format!("Invalid template id '{}'", id));
    }
    let path = dir.join(format!("{}.json", id));
    if !path.is_file() {
        return Err(format!("No custom template '{}' to delete", id));
    }
    std::fs::remove_file(&path).map_err(|e| format!("Failed to delete template file: {}", e))?;
    info!("Deleted custom template '{}' from {:?}", id, path);
    Ok(())
}

/// Delete a custom template override from the user's data directory.
///
/// Only user (custom) templates can be deleted; built-in and bundled templates
/// are never touched. Deleting an override reverts the id to its bundled or
/// built-in definition.
pub fn delete_custom_template(id: &str) -> Result<(), String> {
    let dir = get_custom_templates_dir()
        .ok_or_else(|| "Could not resolve custom templates directory".to_string())?;
    delete_custom_template_in_dir(&dir, id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_builtin_template() {
        let template = get_template("daily_standup");
        assert!(template.is_ok());

        let template = template.unwrap();
        assert_eq!(template.name, "Daily Standup");
        assert!(!template.sections.is_empty());
    }

    #[test]
    fn test_get_nonexistent_template() {
        let result = get_template("nonexistent_template");
        assert!(result.is_err());
    }

    #[test]
    fn test_list_template_ids() {
        let ids = list_template_ids();
        assert!(ids.contains(&"daily_standup".to_string()));
        assert!(ids.contains(&"standard_meeting".to_string()));
    }

    #[test]
    fn test_validate_invalid_json() {
        let result = validate_and_parse_template("invalid json");
        assert!(result.is_err());
    }

    #[test]
    fn test_is_valid_template_id() {
        assert!(is_valid_template_id("daily_standup"));
        assert!(is_valid_template_id("my-template-2"));
        assert!(!is_valid_template_id(""));
        assert!(!is_valid_template_id("../evil"));
        assert!(!is_valid_template_id("has space"));
        assert!(!is_valid_template_id("UPPER"));
    }

    #[test]
    fn test_save_and_delete_custom_template_roundtrip() {
        let dir =
            std::env::temp_dir().join(format!("briefli_tpl_roundtrip_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let json = r#"{"name":"T","description":"D","sections":[{"title":"S","instruction":"I","format":"paragraph"}]}"#;

        assert!(save_custom_template_to_dir(&dir, "custom_x", json).is_ok());
        let path = dir.join("custom_x.json");
        assert!(path.is_file());
        let parsed = validate_and_parse_template(&std::fs::read_to_string(&path).unwrap());
        assert!(parsed.is_ok());

        assert!(delete_custom_template_in_dir(&dir, "custom_x").is_ok());
        assert!(!path.is_file());
        // Deleting a missing custom template is an error.
        assert!(delete_custom_template_in_dir(&dir, "custom_x").is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_save_rejects_invalid_id_and_json() {
        let dir =
            std::env::temp_dir().join(format!("briefli_tpl_invalid_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let good = r#"{"name":"T","description":"D","sections":[{"title":"S","instruction":"I","format":"paragraph"}]}"#;

        // Invalid id (path traversal) is rejected before any write.
        assert!(save_custom_template_to_dir(&dir, "../evil", good).is_err());
        // Invalid JSON is rejected.
        assert!(save_custom_template_to_dir(&dir, "ok_id", "not json").is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_save_replaces_existing_template_without_leaving_temp_files() {
        let dir =
            std::env::temp_dir().join(format!("briefli_tpl_replace_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let first = r#"{"name":"First","description":"D","sections":[{"title":"S","instruction":"I","format":"paragraph"}]}"#;
        let second = r#"{"name":"Second","description":"D","sections":[{"title":"S","instruction":"I","format":"paragraph"}]}"#;

        save_custom_template_to_dir(&dir, "replace_me", first).unwrap();
        save_custom_template_to_dir(&dir, "replace_me", second).unwrap();

        let saved = std::fs::read_to_string(dir.join("replace_me.json")).unwrap();
        assert_eq!(validate_and_parse_template(&saved).unwrap().name, "Second");
        let leftover_files: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|entry| entry.file_name() != "replace_me.json")
            .collect();
        assert!(leftover_files.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
