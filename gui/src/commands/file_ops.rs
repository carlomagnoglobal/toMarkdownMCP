/// File operations: duplicate, move, create, etc.
use std::path::{Path, PathBuf};

/// Backend implementation: duplicate a file with smart naming.
///
/// Logic:
/// 1. Check file exists (error if not)
/// 2. Get parent directory
/// 3. Parse filename and extension
/// 4. Find available duplicate name:
///    - First try: `filename copy.ext`
///    - If exists: `filename copy (2).ext`, `filename copy (3).ext`, etc.
///    - Up to 100 attempts, then error
/// 5. Copy file via std::fs::copy
/// 6. Return PathBuf to new file
pub fn duplicate_file_impl(path: &Path) -> Result<PathBuf, String> {
    // Check file exists
    if !path.is_file() {
        return Err(format!("File not found: {}", path.display()));
    }

    // Get parent directory
    let parent = path.parent().unwrap_or(Path::new("."));

    // Parse filename and extension
    let file_name = path.file_name().and_then(|n| n.to_str()).ok_or_else(|| "Invalid filename".to_string())?;

    let (stem, ext) = match file_name.rsplit_once('.') {
        Some((s, e)) => (s.to_string(), format!(".{}", e)),
        None => (file_name.to_string(), String::new()),
    };

    // Find available duplicate name
    // First try: filename copy.ext
    let mut new_name = format!("{} copy{}", stem, ext);
    let mut new_path = parent.join(&new_name);

    // If it exists, try copy (2), copy (3), etc. up to 100 attempts
    let mut counter = 2;
    while new_path.exists() && counter <= 100 {
        new_name = format!("{} copy ({}){}", stem, counter, ext);
        new_path = parent.join(&new_name);
        counter += 1;
    }

    // If we exhausted all attempts, error
    if counter > 100 {
        return Err("Could not find available filename after 100 attempts".to_string());
    }

    // Copy file
    std::fs::copy(path, &new_path).map_err(|e| format!("Failed to copy file: {}", e))?;

    // Return PathBuf to new file
    Ok(new_path)
}

/// Tauri command to duplicate a file.
/// Accepts file path as String, returns path to duplicate as String.
#[tauri::command]
pub async fn duplicate_file(path: String) -> Result<String, String> {
    let p = PathBuf::from(&path);
    duplicate_file_impl(&p).map(|pb| pb.to_string_lossy().into_owned())
}
