use anyhow::{Context, Result, anyhow};
use std::io::{BufReader, Read, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

const WORKER_SCRIPT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tts_worker.py");

fn write_text_request(writer: &mut impl Write, text: &str) -> Result<()> {
    let text_bytes = text.as_bytes();
    let len = u32::try_from(text_bytes.len()).context("tts request too large")?;
    writer.write_all(&len.to_le_bytes())?;
    writer.write_all(text_bytes)?;
    Ok(())
}

fn read_audio_frame(reader: &mut impl Read) -> Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut data = vec![0u8; len];
    reader.read_exact(&mut data)?;
    Ok(data)
}

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
        write_text_request(&mut self.stdin, text)?;
        self.stdin.flush()?;

        loop {
            let data = read_audio_frame(&mut self.stdout)?;
            if data.is_empty() {
                break;
            }
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

#[cfg(test)]
mod tests {
    use super::{read_audio_frame, write_text_request};
    use std::io::Cursor;

    fn decode_text_request_frame(bytes: &[u8]) -> String {
        let (len_buf, payload) = bytes.split_at(4);
        let len = u32::from_le_bytes(len_buf.try_into().unwrap()) as usize;
        assert_eq!(payload.len(), len);
        String::from_utf8(payload.to_vec()).unwrap()
    }

    #[test]
    fn write_text_request_preserves_embedded_newlines() {
        let text = "first line\nsecond line\n\nfinal line";
        let mut bytes = Vec::new();

        write_text_request(&mut bytes, text).unwrap();

        assert_eq!(decode_text_request_frame(&bytes), text);
    }

    #[test]
    fn write_text_request_encodes_empty_text_as_one_request() {
        let mut bytes = Vec::new();

        write_text_request(&mut bytes, "").unwrap();

        assert_eq!(bytes, 0u32.to_le_bytes());
    }

    #[test]
    fn read_audio_frame_reads_zero_length_end_marker() {
        let mut cursor = Cursor::new(0u32.to_le_bytes().to_vec());

        let frame = read_audio_frame(&mut cursor).unwrap();

        assert!(frame.is_empty());
    }
}
