//! Host-owned model storage and preparation. The engine only loads local files.
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

const ID: &str = "parakeet-unified-en";
const NAME: &str = "Parakeet Unified English";
const FILE: &str = "parakeet-unified-en-0.6b-Q8_0.gguf";
const SIZE: u64 = 731_357_568;
const SHA256: &str = "4b50b6dd862bf6e346929aaf4f5eaacec003bfa3f56462d6c874b41ef2f38795";
const URL: &str = "https://huggingface.co/handy-computer/parakeet-unified-en-0.6b-gguf/resolve/843b1b175ace04f5a89d379c79e2518c0d065b25/parakeet-unified-en-0.6b-Q8_0.gguf";

struct Artifact {
    id: &'static str,
    name: &'static str,
    file: &'static str,
    size: u64,
    digest: &'static str,
    url: &'static str,
    languages: &'static str,
}

const CATALOG: &[Artifact] = &[
    Artifact { id: ID, name: NAME, file: FILE, size: SIZE, digest: SHA256, url: URL, languages: "English" },
    Artifact {
        id: "parakeet-v3", name: "Parakeet v3", file: "parakeet-tdt-0.6b-v3-Q8_0.gguf",
        size: 739_508_576, digest: "5859f77944efcd8eafa23a6350731960b2b55b2203df51f319665c807d802cc7",
        url: "https://huggingface.co/handy-computer/parakeet-tdt-0.6b-v3-gguf/resolve/85ac09ea12fc4b1112fa76810059364bc6adc9de/parakeet-tdt-0.6b-v3-Q8_0.gguf", languages: "25 European languages",
    },
    Artifact {
        id: "whisper-large-v3-turbo", name: "Whisper large-v3-turbo", file: "whisper-large-v3-turbo-Q8_0.gguf",
        size: 886_381_824, digest: "d5e65f2b0828802ae2c231673d31982cebe3a778c95d9494a9f3efee6bd17448",
        url: "https://huggingface.co/handy-computer/whisper-large-v3-turbo-gguf/resolve/d222c9f621c1128299248f2ded4d8a1820519780/whisper-large-v3-turbo-Q8_0.gguf", languages: "Multilingual",
    },
];

fn artifact(id: &str) -> Option<&'static Artifact> {
    CATALOG.iter().find(|model| model.id == id)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    id: String,
    name: String,
    size: u64,
    installed: bool,
    active: bool,
    recommended: bool,
    languages: Option<&'static str>,
    partial_size: u64,
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    model_id: Option<String>,
    busy: bool,
    stage: String,
    downloaded: u64,
    total: u64,
    error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    entries: Vec<Entry>,
    progress: Progress,
}

pub struct Models {
    directory: PathBuf,
    progress: Mutex<Progress>,
    generation: AtomicU64,
}

impl Models {
    pub fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            progress: Mutex::new(Progress::default()),
            generation: AtomicU64::new(0),
        }
    }

    fn begin(&self) -> Result<u64, String> {
        let mut state = self.progress.lock().unwrap();
        if state.busy {
            return Err("A model operation is already in progress.".into());
        }
        let id = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        *state = Progress {
            busy: true,
            stage: "preparing".into(),
            ..Progress::default()
        };
        Ok(id)
    }

    fn check(&self, id: u64) -> Result<(), String> {
        if self.generation.load(Ordering::SeqCst) != id {
            return Err("Cancelled. Any partial download is kept for resume.".into());
        }
        Ok(())
    }

    fn stage(&self, stage: &str, downloaded: u64, total: u64) {
        let mut state = self.progress.lock().unwrap();
        state.stage = stage.into();
        state.downloaded = downloaded;
        state.total = total;
    }

    fn snapshot(&self, selected: &Path) -> Result<Snapshot, String> {
        let mut entries: Vec<_> = CATALOG
            .iter()
            .map(|model| {
                let downloaded = self.directory.join(model.file);
                Entry {
                    id: model.id.into(),
                    name: model.name.into(),
                    size: model.size,
                    installed: downloaded.is_file(),
                    active: selected == downloaded && downloaded.is_file(),
                    recommended: model.id == ID,
                    languages: Some(model.languages),
                    partial_size: fs::metadata(
                        self.directory.join(format!("{}.partial", model.file)),
                    )
                    .map(|m| m.len())
                    .unwrap_or(0),
                }
            })
            .collect();
        let imported = self.directory.join("imports");
        if imported.is_dir() {
            for item in fs::read_dir(&imported).map_err(|e| e.to_string())? {
                let path = item.map_err(|e| e.to_string())?.path();
                if path.extension().is_some_and(|ext| ext == "gguf") && path.is_file() {
                    let filename = path.file_name().unwrap().to_string_lossy().into_owned();
                    let name = filename
                        .trim_end_matches(".gguf")
                        .rsplit_once('-')
                        .map(|(name, _)| name)
                        .unwrap_or(&filename)
                        .to_string();
                    entries.push(Entry {
                        id: format!("import:{filename}"),
                        name,
                        size: fs::metadata(&path).map_err(|e| e.to_string())?.len(),
                        installed: true,
                        active: selected == path,
                        recommended: false,
                        languages: None,
                        partial_size: 0,
                    });
                }
            }
        }
        entries[CATALOG.len()..].sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        // Preserve the pre-manager file selection without moving or deleting it.
        if selected.is_file() && !entries.iter().any(|entry| entry.active) {
            entries.push(Entry {
                id: "existing".into(),
                name: selected
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
                size: fs::metadata(selected).map_err(|e| e.to_string())?.len(),
                installed: true,
                active: true,
                recommended: false,
                languages: None,
                partial_size: 0,
            });
        }
        Ok(Snapshot {
            entries,
            progress: self.progress.lock().unwrap().clone(),
        })
    }

    fn path_for(&self, id: &str, selected: &Path) -> Result<PathBuf, String> {
        if let Some(model) = artifact(id) {
            return Ok(self.directory.join(model.file));
        }
        if id == "existing" {
            return Ok(selected.to_owned());
        }
        if let Some(filename) = id.strip_prefix("import:") {
            if Path::new(filename).file_name().and_then(|s| s.to_str()) == Some(filename)
                && !filename.contains('\\')
                && filename.ends_with(".gguf")
            {
                return Ok(self.directory.join("imports").join(filename));
            }
        }
        Err("Unknown model.".into())
    }

    fn activate(&self, app: &tauri::AppHandle, path: PathBuf, id: u64) -> Result<(), String> {
        self.check(id)?;
        validate_header(&path)?;
        if path.parent() == Some(self.directory.join("imports").as_path()) {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or("Invalid imported model name")?;
            let digest = stem
                .rsplit_once('-')
                .map(|(_, hash)| hash)
                .ok_or("Invalid imported model name")?;
            self.stage("verifying", 0, 0);
            verify(
                &path,
                fs::metadata(&path).map_err(|e| e.to_string())?.len(),
                digest,
                || self.check(id),
            )?;
        }
        self.stage("loading", 0, 0);
        let worker = crate::inference::Worker::prepare(&path, &self.generation, id)?;
        let _commit = self.progress.lock().unwrap();
        self.check(id)?;
        app.state::<crate::session::Sessions>()
            .select_model(path, worker)
    }

    fn import(&self, app: &tauri::AppHandle, id: u64) -> Result<(), String> {
        self.stage("choosing", 0, 0);
        let Some(file) = app
            .dialog()
            .file()
            .add_filter("GGUF speech model", &["gguf"])
            .blocking_pick_file()
        else {
            return Ok(());
        };
        let source = file.into_path().map_err(|e| e.to_string())?;
        self.import_file(&source, id, |path| self.activate(app, path, id))
    }

    fn import_file(
        &self,
        source: &Path,
        id: u64,
        activate: impl FnOnce(PathBuf) -> Result<(), String>,
    ) -> Result<(), String> {
        self.check(id)?;
        validate_header(source)?;
        let dir = self.directory.join("imports");
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let partial = dir.join("import.partial");
        let result = (|| {
            let mut input = File::open(source).map_err(|e| e.to_string())?;
            let size = input.metadata().map_err(|e| e.to_string())?.len();
            let mut output = File::create(&partial).map_err(|e| e.to_string())?;
            let digest = copy_hashed(
                &mut input,
                &mut output,
                || self.check(id),
                |bytes| self.stage("importing", bytes, size),
            )?;
            output.sync_all().map_err(|e| e.to_string())?;
            drop(output);
            validate_header(&partial)?;
            let name: String = source
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .chars()
                .take(80)
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            let path = dir.join(format!("{name}-{digest}.gguf"));
            let created = !path.exists();
            if created {
                fs::rename(&partial, &path).map_err(|e| e.to_string())?;
            }
            let result = self.check(id).and_then(|()| activate(path.clone()));
            if result.is_err() && created {
                let _ = fs::remove_file(path);
            }
            result
        })();
        let _ = fs::remove_file(partial);
        result
    }

    fn download(&self, id: u64, model: &Artifact) -> Result<PathBuf, String> {
        fs::create_dir_all(&self.directory).map_err(|e| e.to_string())?;
        let path = self.directory.join(model.file);
        if path.is_file() {
            self.stage("verifying", 0, model.size);
            if verify(&path, model.size, model.digest, || self.check(id)).is_ok() {
                return Ok(path);
            }
            self.check(id)?;
        }
        let partial = self.directory.join(format!("{}.partial", model.file));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        runtime.block_on(async {
            tokio::select! {
                result = transfer(model.url, &partial, model.size, |bytes| self.stage("downloading", bytes, model.size)) => result,
                _ = async {
                    while self.check(id).is_ok() { tokio::time::sleep(Duration::from_millis(100)).await; }
                } => self.check(id),
            }
        })?;
        self.stage("verifying", model.size, model.size);
        if let Err(error) = verify(&partial, model.size, model.digest, || self.check(id)) {
            // Cancellation keeps a complete partial; corrupt bytes must be fetched again.
            if self.check(id).is_ok() {
                let _ = fs::remove_file(&partial);
            }
            return Err(error);
        }
        self.check(id)?;
        fs::rename(&partial, &path).map_err(|e| e.to_string())?;
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader},
        net::TcpListener,
        thread,
    };

    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "chime-download-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::SeqCst)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    #[ignore = "uses the real Hugging Face endpoint and a local pinned model fixture"]
    fn recommended_model_download_smoke() {
        let fixture = PathBuf::from(
            std::env::var_os("CHIME_MODEL_FIXTURE").expect("Set CHIME_MODEL_FIXTURE"),
        );
        verify(&fixture, SIZE, SHA256, || Ok(())).unwrap();
        let temp = Temp::new();
        let manager = Models::new(temp.0.clone());
        let partial = temp.0.join(format!("{FILE}.partial"));
        // Exercise the actual CDN's resume response without fetching another 731 MB.
        std::io::copy(
            &mut File::open(fixture).unwrap().take(SIZE - 1024 * 1024),
            &mut File::create(&partial).unwrap(),
        )
        .unwrap();
        let id = manager.begin().unwrap();
        let path = manager.download(id, &CATALOG[0]).unwrap();
        assert!(!partial.exists());
        verify(&path, SIZE, SHA256, || Ok(())).unwrap();
        chime_engine::LocalTranscriber::load(&path).expect("the pinned catalog model must load");
        // A second preparation uses the completed local file.
        assert_eq!(manager.download(id, &CATALOG[0]).unwrap(), path);
    }

    fn server(response: &'static str) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/model", listener.local_addr().unwrap());
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request = String::new();
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                request.push_str(&line);
                if line == "\r\n" || line.is_empty() {
                    break;
                }
            }
            stream.write_all(response.as_bytes()).unwrap();
            request
        });
        (url, worker)
    }

    fn fetch(url: &str, path: &Path, size: u64) -> Result<(), String> {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(transfer(url, path, size, |_| {}))
    }

    #[test]
    fn resumes_a_partial_without_duplicating_bytes() {
        let temp = Temp::new();
        let path = temp.0.join("model.partial");
        fs::write(&path, b"abc").unwrap();
        let (url, worker) = server("HTTP/1.1 206 Partial Content\r\nContent-Length: 3\r\nContent-Range: bytes 3-5/6\r\nConnection: close\r\n\r\ndef");
        fetch(&url, &path, 6).unwrap();
        assert!(worker
            .join()
            .unwrap()
            .to_lowercase()
            .contains("range: bytes=3-"));
        assert_eq!(fs::read(&path).unwrap(), b"abcdef");
    }

    #[test]
    fn ignored_range_restarts_and_invalid_range_preserves_partial() {
        let temp = Temp::new();
        let path = temp.0.join("model.partial");
        fs::write(&path, b"abc").unwrap();
        let (url, worker) = server("HTTP/1.1 206 Partial Content\r\nContent-Length: 3\r\nContent-Range: bytes 1-3/6\r\nConnection: close\r\n\r\ndef");
        assert!(fetch(&url, &path, 6).is_err());
        worker.join().unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"abc");
        let (url, worker) =
            server("HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nabcdef");
        fetch(&url, &path, 6).unwrap();
        worker.join().unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"abcdef");
    }

    #[test]
    fn interrupted_transfer_keeps_bytes_for_retry() {
        let temp = Temp::new();
        let path = temp.0.join("model.partial");
        let (url, worker) = server("HTTP/1.1 200 OK\r\nConnection: close\r\n\r\nabc");
        assert!(fetch(&url, &path, 6).is_err());
        worker.join().unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"abc");
    }

    #[test]
    fn checksum_and_cancellation_prevent_accepting_bad_files() {
        let temp = Temp::new();
        let path = temp.0.join("model.partial");
        fs::write(&path, b"abc").unwrap();
        let digest = format!("{:x}", Sha256::digest(b"abc"));
        assert!(verify(&path, 3, &digest, || Ok(())).is_ok());
        assert!(verify(&path, 4, &digest, || Ok(())).is_err());
        assert!(verify(&path, 3, "wrong", || Ok(())).is_err());
        assert!(verify(&path, 3, &digest, || Err("Cancelled".into())).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"abc");
    }

    #[test]
    fn fresh_install_is_downloadable_and_external_selection_is_preserved() {
        let temp = Temp::new();
        let manager = Models::new(temp.0.join("models"));
        let snapshot = manager.snapshot(&manager.directory.join(FILE)).unwrap();
        assert!(!snapshot.entries[0].installed);
        assert!(!snapshot.entries[0].active);
        let external = temp.0.join("external.gguf");
        fs::write(&external, b"GGUF").unwrap();
        let snapshot = manager.snapshot(&external).unwrap();
        assert_eq!(snapshot.entries.len(), CATALOG.len() + 1);
        assert!(snapshot.entries.last().unwrap().active);
        assert!(manager
            .path_for("import:../../outside.gguf", &external)
            .is_err());
        assert!(manager
            .path_for("import:C:\\outside.gguf", &external)
            .is_err());
        let id = manager.begin().unwrap();
        assert!(manager.begin().is_err());
        manager.generation.fetch_add(1, Ordering::SeqCst);
        assert!(manager.check(id).is_err());
        assert!(external.exists());
    }

    #[test]
    fn rejects_incomplete_or_non_gguf_imports() {
        let temp = Temp::new();
        let path = temp.0.join("bad.gguf");
        for bytes in [b"GGUF".to_vec(), vec![0; 24]] {
            fs::write(&path, bytes).unwrap();
            assert!(validate_header(&path).is_err());
        }
    }

    #[test]
    fn imports_keep_originals_and_rollback_only_new_copies() {
        let temp = Temp::new();
        let manager = Models::new(temp.0.join("managed"));
        let id = manager.begin().unwrap();
        let original = temp.0.join("speech.gguf");
        let mut bytes = b"GGUF".to_vec();
        bytes.extend(3u32.to_le_bytes());
        bytes.extend(1u64.to_le_bytes());
        bytes.extend(0u64.to_le_bytes());
        fs::write(&original, &bytes).unwrap();

        // Simulate a worker rejecting a file after it passed the header check.
        assert!(manager
            .import_file(&original, id, |_| Err("Unsupported speech model".into()))
            .is_err());
        assert_eq!(
            fs::read_dir(manager.directory.join("imports"))
                .unwrap()
                .count(),
            0
        );
        assert_eq!(fs::read(&original).unwrap(), bytes);

        let mut imported = PathBuf::new();
        manager
            .import_file(&original, id, |path| {
                imported = path;
                Ok(())
            })
            .unwrap();
        assert_ne!(imported, original);
        assert_eq!(fs::read(&imported).unwrap(), bytes);

        // Reimporting the same file must not delete an existing managed copy on failure.
        assert!(manager
            .import_file(&original, id, |_| Err("Loading failed".into()))
            .is_err());
        assert_eq!(fs::read(&imported).unwrap(), bytes);
        assert_eq!(fs::read(&original).unwrap(), bytes);
        assert_eq!(
            fs::read_dir(manager.directory.join("imports"))
                .unwrap()
                .count(),
            1
        );
    }
}

fn validate_header(path: &Path) -> Result<(), String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut header = [0u8; 24];
    file.read_exact(&mut header)
        .map_err(|_| "The model file is incomplete.".to_string())?;
    if &header[..4] != b"GGUF"
        || !matches!(u32::from_le_bytes(header[4..8].try_into().unwrap()), 2 | 3)
    {
        return Err("Choose a GGUF v2 or v3 speech model compatible with transcribe.cpp.".into());
    }
    if u64::from_le_bytes(header[8..16].try_into().unwrap()) == 0 {
        return Err("The model contains no tensors.".into());
    }
    Ok(())
}

fn copy_hashed(
    input: &mut impl Read,
    output: &mut impl Write,
    check: impl Fn() -> Result<(), String>,
    progress: impl Fn(u64),
) -> Result<String, String> {
    let mut hash = Sha256::new();
    let mut buffer = vec![0; 1024 * 1024];
    let mut count = 0;
    loop {
        check()?;
        let read = input.read(&mut buffer).map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        output
            .write_all(&buffer[..read])
            .map_err(|e| e.to_string())?;
        hash.update(&buffer[..read]);
        count += read as u64;
        progress(count);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn verify(
    path: &Path,
    size: u64,
    expected: &str,
    check: impl Fn() -> Result<(), String>,
) -> Result<(), String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() != size {
        return Err("Incomplete model download. Retry to resume.".into());
    }
    let hash = copy_hashed(&mut file, &mut std::io::sink(), check, |_| {})?;
    if hash != expected {
        return Err("Model checksum did not match. Download it again.".into());
    }
    Ok(())
}

/// Pinned artifacts permit resume without trusting a mutable URL or ETag.
async fn transfer(
    url: &str,
    partial: &Path,
    size: u64,
    progress: impl Fn(u64),
) -> Result<(), String> {
    let mut offset = fs::metadata(partial).map(|m| m.len()).unwrap_or(0);
    if offset > size {
        fs::remove_file(partial).map_err(|e| e.to_string())?;
        offset = 0;
    }
    progress(offset);
    if offset == size {
        return Ok(());
    }
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .read_timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let mut request = client
        .get(url)
        .header(reqwest::header::ACCEPT_ENCODING, "identity");
    if offset > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={offset}-"));
    }
    let mut response = request
        .send()
        .await
        .map_err(|e| format!("Download failed: {e}. Retry to resume."))?;
    let status = response.status();
    if status == reqwest::StatusCode::PARTIAL_CONTENT {
        let expected = format!("bytes {offset}-{}/{size}", size - 1);
        if response
            .headers()
            .get(reqwest::header::CONTENT_RANGE)
            .and_then(|v| v.to_str().ok())
            != Some(expected.as_str())
        {
            return Err("Server returned an unexpected download range. Retry later.".into());
        }
    } else if status == reqwest::StatusCode::OK {
        // A server may ignore Range. Restart instead of appending duplicate bytes.
        offset = 0;
    } else {
        return Err(format!("Model server returned {status}. Retry later."));
    }
    let mut output = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(offset == 0)
        .append(offset > 0)
        .open(partial)
        .map_err(|e| e.to_string())?;
    progress(offset);
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("Download interrupted: {e}. Retry to resume."))?
    {
        if offset + chunk.len() as u64 > size {
            return Err("Model download exceeds the expected size.".into());
        }
        output
            .write_all(&chunk)
            .map_err(|e| format!("Could not save model: {e}"))?;
        offset += chunk.len() as u64;
        progress(offset);
    }
    output.sync_all().map_err(|e| e.to_string())?;
    if offset != size {
        return Err("Download interrupted. Retry to resume.".into());
    }
    Ok(())
}

#[tauri::command]
pub fn models_open_info(app: tauri::AppHandle, page: String) -> Result<(), String> {
    let url = match page.as_str() {
        "source" => "https://huggingface.co/handy-computer/parakeet-unified-en-0.6b-gguf",
        "license" => {
            "https://www.nvidia.com/en-us/agreements/enterprise-software/nvidia-open-model-license/"
        }
        _ => return Err("Unknown model information page.".into()),
    };
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn models_snapshot(app: tauri::AppHandle) -> Result<Snapshot, String> {
    app.state::<Arc<Models>>()
        .snapshot(&app.state::<crate::session::Sessions>().model_path())
}

#[tauri::command]
pub fn models_cancel(models: tauri::State<'_, Arc<Models>>) {
    let _state = models.progress.lock().unwrap();
    models.generation.fetch_add(1, Ordering::SeqCst);
}

#[tauri::command]
pub fn models_start(
    app: tauri::AppHandle,
    action: String,
    model_id: Option<String>,
) -> Result<(), String> {
    if !matches!(action.as_str(), "download" | "import" | "use") {
        return Err("Unknown model action.".into());
    }
    let models = app.state::<Arc<Models>>().inner().clone();
    let id = models.begin()?;
    models.progress.lock().unwrap().model_id = if action == "download" {
        Some(model_id.as_deref().unwrap_or(ID).to_string())
    } else {
        model_id.clone()
    };
    tauri::async_runtime::spawn_blocking(move || {
        let result = match action.as_str() {
            "import" => models.import(&app, id),
            "download" => artifact(model_id.as_deref().unwrap_or(ID))
                .ok_or_else(|| "Unknown downloadable model.".to_string())
                .and_then(|model| models.download(id, model))
                .and_then(|path| models.activate(&app, path, id)),
            _ => models
                .path_for(
                    model_id.as_deref().unwrap_or(""),
                    &app.state::<crate::session::Sessions>().model_path(),
                )
                .and_then(|path| {
                    if let Some(model) = model_id.as_deref().and_then(artifact) {
                        models.stage("verifying", 0, model.size);
                        verify(&path, model.size, model.digest, || models.check(id))?;
                    }
                    models.activate(&app, path, id)
                }),
        };
        let mut state = models.progress.lock().unwrap();
        state.busy = false;
        state.stage = if result.is_ok() { "ready" } else { "stopped" }.into();
        state.error = result.err();
    });
    Ok(())
}
