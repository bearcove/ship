use anyhow::{Result, anyhow};
use std::io::{BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

const WORKER_SCRIPT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tts_worker.py");

pub struct KyutaiTtsModel {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl KyutaiTtsModel {
    /// Spawn the Python TTS worker. Sync; call from `tokio::task::spawn_blocking`.
    pub fn load() -> Result<Self> {
        tracing::info!("spawning pocket-tts worker");
        let mut child = Command::new("uv")
            .args(["run", WORKER_SCRIPT])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;

        let stdin = child.stdin.take().ok_or_else(|| anyhow!("no stdin"))?;
        let stdout = BufReader::new(child.stdout.take().ok_or_else(|| anyhow!("no stdout"))?);

        Ok(Self {
            child,
            stdin,
            stdout,
        })
    }

    /// Stream synthesized 24kHz mono f32 LE PCM chunks for `text`.
    /// `on_chunk` is called for each decoded audio chunk as it's generated.
    /// Runs synchronously; call from `tokio::task::spawn_blocking`.
    pub fn speak(&mut self, text: &str, mut on_chunk: impl FnMut(Vec<u8>)) -> Result<()> {
        // Send text line to worker
        writeln!(self.stdin, "{}", text)?;
        self.stdin.flush()?;

        // Read frames until end-of-utterance (zero-length frame)
        loop {
            let mut len_buf = [0u8; 4];
            self.stdout.read_exact(&mut len_buf)?;
            let len = u32::from_le_bytes(len_buf) as usize;
            if len == 0 {
                break;
            }
            let mut data = vec![0u8; len];
            self.stdout.read_exact(&mut data)?;
            on_chunk(data);
        }

        Ok(())
    }
}

impl Drop for KyutaiTtsModel {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}
