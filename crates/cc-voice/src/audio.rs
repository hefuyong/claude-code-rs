//! Audio capture for voice mode.
//!
//! Supports multiple backends: a native pipe-based reader, `sox` (cross-platform),
//! and `arecord` (Linux ALSA). The best available backend is auto-detected.

use cc_error::{CcError, CcResult};
use tokio::process::Command;
use tokio::sync::mpsc;

/// Audio recording backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioBackend {
    /// Read raw PCM data from a named pipe or stdin (cross-platform fallback).
    Native,
    /// Use the SoX `rec` command (available on most platforms).
    Sox,
    /// Use ALSA `arecord` (Linux only).
    Arecord,
}

/// Audio sample format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    /// Signed 16-bit little-endian PCM.
    Pcm16,
    /// 32-bit IEEE float.
    Float32,
}

/// Audio capture parameters.
#[derive(Debug, Clone)]
pub struct AudioConfig {
    /// Sample rate in Hz (default 16000).
    pub sample_rate: u32,
    /// Number of channels (default 1 = mono).
    pub channels: u16,
    /// Sample format (default PCM16).
    pub format: AudioFormat,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16_000,
            channels: 1,
            format: AudioFormat::Pcm16,
        }
    }
}

/// A stream of audio chunks produced by an active recording.
pub struct AudioStream {
    rx: mpsc::Receiver<Vec<u8>>,
}

impl AudioStream {
    /// Receive the next chunk of audio data, or `None` if the stream ended.
    pub async fn next_chunk(&mut self) -> Option<Vec<u8>> {
        self.rx.recv().await
    }
}

/// Audio capture handle.
///
/// Spawns an external process (`sox` or `arecord`) or reads from a pipe to
/// capture microphone audio and emit PCM chunks.
pub struct AudioCapture {
    backend: AudioBackend,
    config: AudioConfig,
    recording: bool,
    /// Handle to the spawned recording process (if any).
    child: Option<tokio::process::Child>,
    /// Sender kept so we can drop it to close the stream.
    tx: Option<mpsc::Sender<Vec<u8>>>,
}

impl AudioCapture {
    /// Create a new audio capture using the best available backend.
    pub fn new(config: AudioConfig) -> CcResult<Self> {
        let backend = Self::detect_backend();
        tracing::info!(?backend, "audio capture backend selected");
        Ok(Self {
            backend,
            config,
            recording: false,
            child: None,
            tx: None,
        })
    }

    /// Detect the best recording backend available on this system.
    ///
    /// Checks for `sox` first (cross-platform), then `arecord` (Linux),
    /// and falls back to native pipe reading.
    pub fn detect_backend() -> AudioBackend {
        if which_sync("sox") {
            AudioBackend::Sox
        } else if which_sync("arecord") {
            AudioBackend::Arecord
        } else {
            AudioBackend::Native
        }
    }

    /// Start recording audio. Returns an [`AudioStream`] that yields PCM chunks.
    pub async fn start_recording(&mut self) -> CcResult<AudioStream> {
        if self.recording {
            return Err(CcError::Internal("already recording".into()));
        }

        let (tx, rx) = mpsc::channel::<Vec<u8>>(64);

        match self.backend {
            AudioBackend::Sox => {
                self.spawn_sox(&tx).await?;
            }
            AudioBackend::Arecord => {
                self.spawn_arecord(&tx).await?;
            }
            AudioBackend::Native => {
                self.spawn_native_reader(&tx).await?;
            }
        }

        self.recording = true;
        self.tx = Some(tx);
        Ok(AudioStream { rx })
    }

    /// Stop the current recording and return the remaining buffered PCM data.
    pub async fn stop_recording(&mut self) -> CcResult<Vec<u8>> {
        if !self.recording {
            return Err(CcError::Internal("not recording".into()));
        }

        // Kill the subprocess if one is running.
        if let Some(ref mut child) = self.child {
            let _ = child.kill().await;
        }
        self.child = None;

        // Drop the sender so the stream ends.
        self.tx.take();
        self.recording = false;

        tracing::debug!("audio recording stopped");
        Ok(Vec::new())
    }

    /// Whether a recording is currently in progress.
    pub fn is_recording(&self) -> bool {
        self.recording
    }

    // --- backend spawning helpers ---

    async fn spawn_sox(&mut self, tx: &mpsc::Sender<Vec<u8>>) -> CcResult<()> {
        let rate = self.config.sample_rate.to_string();
        let channels = self.config.channels.to_string();
        let encoding = match self.config.format {
            AudioFormat::Pcm16 => "signed-integer",
            AudioFormat::Float32 => "floating-point",
        };
        let bits = match self.config.format {
            AudioFormat::Pcm16 => "16",
            AudioFormat::Float32 => "32",
        };

        let mut child = Command::new("sox")
            .args([
                "-d",            // default audio device
                "-t", "raw",     // raw PCM output
                "-r", &rate,
                "-c", &channels,
                "-e", encoding,
                "-b", bits,
                "-",             // stdout
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| CcError::Internal(format!("failed to spawn sox: {e}")))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| CcError::Internal("sox stdout not available".into()))?;

        self.child = Some(child);
        Self::pipe_stdout_to_channel(stdout, tx.clone());
        Ok(())
    }

    async fn spawn_arecord(&mut self, tx: &mpsc::Sender<Vec<u8>>) -> CcResult<()> {
        let rate = self.config.sample_rate.to_string();
        let channels = self.config.channels.to_string();
        let format = match self.config.format {
            AudioFormat::Pcm16 => "S16_LE",
            AudioFormat::Float32 => "FLOAT_LE",
        };

        let mut child = Command::new("arecord")
            .args([
                "-f", format,
                "-r", &rate,
                "-c", &channels,
                "-t", "raw",
                "-",
            ])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| CcError::Internal(format!("failed to spawn arecord: {e}")))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| CcError::Internal("arecord stdout not available".into()))?;

        self.child = Some(child);
        Self::pipe_stdout_to_channel(stdout, tx.clone());
        Ok(())
    }

    async fn spawn_native_reader(&mut self, tx: &mpsc::Sender<Vec<u8>>) -> CcResult<()> {
        // Native backend: read from stdin in a blocking thread.
        let tx = tx.clone();
        tokio::task::spawn_blocking(move || {
            use std::io::Read;
            let stdin = std::io::stdin();
            let mut handle = stdin.lock();
            let mut buf = vec![0u8; 4096];
            loop {
                match handle.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.blocking_send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(())
    }

    /// Spawn a task that reads from a child process stdout and sends chunks
    /// into the mpsc channel.
    fn pipe_stdout_to_channel(
        mut stdout: tokio::process::ChildStdout,
        tx: mpsc::Sender<Vec<u8>>,
    ) {
        tokio::spawn(async move {
            use tokio::io::AsyncReadExt;
            let mut buf = vec![0u8; 4096];
            loop {
                match stdout.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
    }
}

/// Synchronous check whether an executable exists on PATH.
fn which_sync(name: &str) -> bool {
    // Use `which` on Unix or `where` on Windows to test availability.
    #[cfg(unix)]
    let result = std::process::Command::new("which")
        .arg(name)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    #[cfg(windows)]
    let result = std::process::Command::new("where")
        .arg(name)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    #[cfg(not(any(unix, windows)))]
    let result: Result<std::process::ExitStatus, std::io::Error> =
        Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "unsupported platform"));

    result.map(|s| s.success()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_audio_config() {
        let cfg = AudioConfig::default();
        assert_eq!(cfg.sample_rate, 16_000);
        assert_eq!(cfg.channels, 1);
        assert_eq!(cfg.format, AudioFormat::Pcm16);
    }

    #[test]
    fn detect_backend_returns_a_variant() {
        // Should always succeed without panicking.
        let _backend = AudioCapture::detect_backend();
    }

    #[test]
    fn new_capture_not_recording() {
        let capture = AudioCapture::new(AudioConfig::default()).unwrap();
        assert!(!capture.is_recording());
    }
}
