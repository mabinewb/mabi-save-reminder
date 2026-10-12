use crate::model::Config;
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

const DEFAULT_SETTINGS: &str = include_str!("../assets/default-settings.ini");

pub fn path_in_appdata(appdata: &Path) -> PathBuf {
    appdata.join("스더저장해").join("settings.ini")
}

pub fn prepare() -> io::Result<(PathBuf, Config)> {
    let appdata = std::env::var_os("APPDATA")
        .filter(|p| !p.is_empty())
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "APPDATA 폴더를 찾을 수 없습니다.")
        })?;
    let path = path_in_appdata(Path::new(&appdata));
    let legacy = std::env::current_exe()?.with_file_name("settings.ini");
    let config = load(&path, Some(&legacy))?;
    Ok((path, config))
}

// Existing AppData settings always win. Legacy settings are copied only on
// startup when the AppData file is absent; the original is left untouched.
pub fn load(path: &Path, legacy: Option<&Path>) -> io::Result<Config> {
    match fs::read_to_string(path) {
        Ok(text) => return Ok(Config::parse(&text)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }

    let text = match legacy.map(fs::read_to_string) {
        Some(Ok(text)) => text,
        Some(Err(e)) if e.kind() != io::ErrorKind::NotFound => return Err(e),
        _ => DEFAULT_SETTINGS.to_owned(),
    };
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "설정 경로가 올바르지 않습니다.",
        )
    })?;
    fs::create_dir_all(parent)?;
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(mut file) => file.write_all(text.as_bytes())?,
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e),
    }
    fs::read_to_string(path).map(|text| Config::parse(&text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Folder(PathBuf);
    impl Folder {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "seudeo-settings-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn first_run_creates_folder_and_default_file_in_appdata() {
        let folder = Folder::new();
        let path = path_in_appdata(&folder.0.join("Roaming 폴더"));
        assert_eq!(path, folder.0.join("Roaming 폴더/스더저장해/settings.ini"));
        let config = load(&path, None).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), DEFAULT_SETTINGS);
        assert_eq!(config.names, Config::default().names);
        assert_eq!(config.path_contains, Config::default().path_contains);
        assert_eq!(config.blink_ms, 650);
        assert_eq!(config.timeout_seconds, 300);
    }
    #[test]
    fn existing_settings_are_preserved_on_restart() {
        let folder = Folder::new();
        let path = path_in_appdata(&folder.0);
        load(&path, None).unwrap();
        let text = "blink_ms=0\ntimeout_seconds=42\n";
        fs::write(&path, text).unwrap();
        let config = load(&path, None).unwrap();
        assert_eq!(config.blink_ms, 0);
        assert_eq!(config.timeout_seconds, 42);
        assert_eq!(fs::read_to_string(path).unwrap(), text);
    }
    #[test]
    fn legacy_settings_are_imported_without_changing_original() {
        let folder = Folder::new();
        let legacy = folder.0.join("settings.ini");
        let text = "process_names=Game.exe\npath_contains=사용자 설치 폴더\nblink_ms=0\n";
        fs::write(&legacy, text).unwrap();
        let path = path_in_appdata(&folder.0.join("Roaming"));
        let config = load(&path, Some(&legacy)).unwrap();
        assert_eq!(config.names, vec!["game.exe"]);
        assert_eq!(config.blink_ms, 0);
        assert_eq!(fs::read_to_string(&path).unwrap(), text);
        assert_eq!(fs::read_to_string(legacy).unwrap(), text);
    }
    #[test]
    fn legacy_file_never_overwrites_existing_appdata_settings() {
        let folder = Folder::new();
        let path = path_in_appdata(&folder.0);
        load(&path, None).unwrap();
        fs::write(&path, "timeout_seconds=42").unwrap();
        let legacy = folder.0.join("settings.ini");
        fs::write(&legacy, "timeout_seconds=99").unwrap();
        assert_eq!(load(&path, Some(&legacy)).unwrap().timeout_seconds, 42);
    }
    #[test]
    fn deleted_settings_are_recreated_when_opening_or_reloading() {
        let folder = Folder::new();
        let path = path_in_appdata(&folder.0);
        load(&path, None).unwrap();
        fs::remove_file(&path).unwrap();
        load(&path, None).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), DEFAULT_SETTINGS);
    }
    #[test]
    fn inaccessible_settings_are_reported_instead_of_silently_reset() {
        let folder = Folder::new();
        fs::write(folder.0.join("스더저장해"), "not a directory").unwrap();
        assert!(load(&path_in_appdata(&folder.0), None).is_err());
    }
}
