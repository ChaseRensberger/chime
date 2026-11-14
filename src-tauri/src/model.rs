use std::{
    io::Read,
    path::{Path, PathBuf},
};

pub fn load(config: &Path, fallback: PathBuf) -> PathBuf {
    std::fs::read(config)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or(fallback)
}

pub fn save(config: &Path, model: &Path) -> Result<(), String> {
    let mut magic = [0; 4];
    std::fs::File::open(model)
        .and_then(|mut file| file.read_exact(&mut magic))
        .map_err(|e| e.to_string())?;
    if &magic != b"GGUF" {
        return Err("Choose a GGUF speech model.".into());
    }
    let parent = config.parent().ok_or("Invalid settings path")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let temporary = config.with_extension("tmp");
    let bytes = serde_json::to_vec(model).map_err(|e| e.to_string())?;
    std::fs::write(&temporary, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(temporary, config).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_selection_preserves_saved_model() {
        let dir = std::env::temp_dir().join(format!("chime-model-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let valid = dir.join("speech.gguf");
        let invalid = dir.join("wrong.gguf");
        let config = dir.join("model.json");
        std::fs::write(&valid, b"GGUFtest").unwrap();
        std::fs::write(&invalid, b"not a model").unwrap();
        save(&config, &valid).unwrap();
        assert!(save(&config, &invalid).is_err());
        assert_eq!(load(&config, PathBuf::new()), valid);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
