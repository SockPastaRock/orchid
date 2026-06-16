use crate::convo::Store;

pub fn create(
    label: Option<String>,
    persona: Option<String>,
    working_dir: Option<String>,
    profile: Option<String>,
) -> Result<serde_json::Value, String> {
    let store = Store::new()?;
    let wd = resolve_working_dir(working_dir)?;
    let meta = store.create(label, Some(wd), persona, profile)?;
    serde_json::to_value(&meta).map_err(|e| e.to_string())
}

pub fn resolve_working_dir(working_dir: Option<String>) -> Result<String, String> {
    match working_dir {
        Some(wd) => Ok(wd),
        None => std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .map_err(|e| format!("failed to get current directory: {}", e)),
    }
}
