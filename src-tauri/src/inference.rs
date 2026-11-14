//! One warm worker, replaced after cancellation, failure, or a model change.
use chime_engine::Transcriber;
use std::{
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const MAX_SAMPLES: usize = 16_000 * 60;
const MAX_RESPONSE: u64 = 64 * 1024;
const DEADLINE: Duration = Duration::from_secs(180);

pub struct Worker {
    model: PathBuf,
    child: Child,
    stdin: Option<ChildStdin>,
    responses: mpsc::Receiver<Result<String, String>>,
    reader: Option<JoinHandle<()>>,
}

impl Worker {
    pub fn prepare(model: &Path, generation: &AtomicU64, id: u64) -> Result<Self, String> {
        let worker = Self::spawn(model)?;
        worker.receive(
            generation,
            id,
            "Model loading timed out. Try a smaller model.",
        )?;
        Ok(worker)
    }

    fn spawn(model: &Path) -> Result<Self, String> {
        let mut child = Command::new(std::env::current_exe().map_err(|e| e.to_string())?)
            .arg("--transcribe-worker")
            .arg(model)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        let stdout = child.stdout.take().unwrap();
        let stdin = child.stdin.take();
        let (tx, responses) = mpsc::channel();
        let reader = thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                let result = (&mut reader).take(MAX_RESPONSE).read_line(&mut line);
                match result {
                    Ok(0) => break,
                    Ok(_) if line.ends_with('\n') => {
                        let parsed = serde_json::from_str::<Result<String, String>>(&line)
                            .map_err(|e| e.to_string())
                            .and_then(|r| r);
                        if tx.send(parsed).is_err() {
                            break;
                        }
                    }
                    _ => {
                        let _ = tx.send(Err("Invalid speech worker response".into()));
                        break;
                    }
                }
            }
        });
        Ok(Self {
            model: model.to_owned(),
            child,
            stdin,
            responses,
            reader: Some(reader),
        })
    }

    fn receive(
        &self,
        generation: &AtomicU64,
        id: u64,
        timeout_message: &str,
    ) -> Result<String, String> {
        let start = Instant::now();
        loop {
            if generation.load(Ordering::SeqCst) != id {
                return Err("Cancelled".into());
            }
            if start.elapsed() >= DEADLINE {
                return Err(timeout_message.into());
            }
            match self.responses.recv_timeout(Duration::from_millis(50)) {
                Ok(response) => return response,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => return Err("Speech worker stopped. Try again.".into()),
            }
        }
    }

    fn run(
        &mut self,
        samples: Vec<f32>,
        generation: &AtomicU64,
        id: u64,
    ) -> Result<String, String> {
        let mut stdin = self.stdin.take().ok_or("Worker input is closed")?;
        let writer = thread::spawn(move || {
            let result = (|| -> std::io::Result<()> {
                stdin.write_all(&(samples.len() as u32).to_le_bytes())?;
                let bytes: Vec<u8> = samples.into_iter().flat_map(f32::to_le_bytes).collect();
                stdin.write_all(&bytes)?;
                stdin.flush()
            })();
            (stdin, result)
        });
        let result = self.receive(
            generation,
            id,
            "Transcription timed out. Try a shorter recording.",
        );
        if result.is_err() {
            let _ = self.child.kill();
        }
        let (stdin, written) = writer.join().map_err(|_| "Worker writer stopped")?;
        self.stdin = Some(stdin);
        written.map_err(|e| e.to_string())?;
        result
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        self.stdin.take();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

pub fn infer(
    worker: &mut Option<Worker>,
    model: &Path,
    samples: Vec<f32>,
    generation: &AtomicU64,
    id: u64,
) -> Result<String, String> {
    let result = (|| {
        if worker.as_ref().is_some_and(|w| w.model != model) {
            *worker = None;
        }
        if worker.is_none() {
            *worker = Some(Worker::prepare(model, generation, id)?);
        }
        worker.as_mut().unwrap().run(samples, generation, id)
    })();
    if result.is_err() {
        *worker = None;
    }
    result
}

fn respond(output: &mut impl Write, result: Result<String, String>) -> std::io::Result<()> {
    serde_json::to_writer(&mut *output, &result)?;
    output.write_all(b"\n")?;
    output.flush()
}

pub fn worker(model: PathBuf) {
    let mut output = std::io::stdout().lock();
    let mut engine = match chime_engine::LocalTranscriber::load(&model) {
        Ok(engine) => engine,
        Err(error) => {
            let _ = respond(&mut output, Err(error.to_string()));
            return;
        }
    };
    if respond(&mut output, Ok(String::new())).is_err() {
        return;
    }
    let mut input = std::io::stdin().lock();
    loop {
        let mut size = [0u8; 4];
        if input.read_exact(&mut size).is_err() {
            return;
        }
        let size = u32::from_le_bytes(size) as usize;
        if size == 0 || size > MAX_SAMPLES {
            let _ = respond(&mut output, Err("Invalid PCM size".into()));
            return;
        }
        let mut bytes = vec![0; size * 4];
        if input.read_exact(&mut bytes).is_err() {
            return;
        }
        let samples: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|v| f32::from_le_bytes(v.try_into().unwrap()))
            .collect();
        let result = engine
            .transcribe(chime_engine::Audio { samples: &samples })
            .map(|t| t.text)
            .map_err(|e| e.to_string());
        if respond(&mut output, result).is_err() {
            return;
        }
    }
}
