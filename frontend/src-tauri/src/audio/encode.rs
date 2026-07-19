use super::ffmpeg::find_ffmpeg_path; // Correct path to encode module
use super::AudioDevice;
use std::io::Write;
use std::sync::Arc;
use std::{
    path::PathBuf,
    process::{Command, Stdio},
};
use tokio::io::AsyncWriteExt;
use tracing::{debug, error};

pub struct AudioInput {
    pub data: Arc<Vec<f32>>,
    pub sample_rate: u32,
    pub channels: u16,
    pub device: Arc<AudioDevice>,
}

pub fn encode_single_audio(
    data: &[u8],
    sample_rate: u32,
    channels: u16,
    output_path: &PathBuf,
) -> anyhow::Result<()> {
    debug!(
        "Starting FFmpeg process for {} bytes of audio data",
        data.len()
    );

    if data.is_empty() {
        return Err(anyhow::anyhow!("No audio data provided for encoding"));
    }

    let ffmpeg_path = find_ffmpeg_path().ok_or_else(|| {
        anyhow::anyhow!("FFmpeg not found. Please install FFmpeg to save recordings.")
    })?;

    debug!("Using FFmpeg at: {:?}", ffmpeg_path);

    let mut command = Command::new(ffmpeg_path);
    command
        .args([
            "-f",
            "f32le",
            "-ar",
            &sample_rate.to_string(),
            "-ac",
            &channels.to_string(),
            "-i",
            "pipe:0",
            "-c:a",
            "aac",
            "-b:a",
            "192k", // Increased from 64k for better audio quality (especially for speech)
            "-profile:a",
            "aac_low", // Use AAC-LC profile for better compatibility
            "-movflags",
            "+faststart", // Optimize for web streaming
            "-f",
            "mp4",
            output_path.to_str().unwrap(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Hide console window on Windows to prevent CMD popup during recording
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    debug!("FFmpeg command: {:?}", command);

    #[allow(clippy::zombie_processes)]
    let mut ffmpeg = command.spawn().expect("Failed to spawn FFmpeg process");
    debug!("FFmpeg process spawned");
    let mut stdin = ffmpeg.stdin.take().expect("Failed to open stdin");

    stdin.write_all(data)?;

    debug!("Dropping stdin");
    drop(stdin);
    debug!("Waiting for FFmpeg process to exit");
    let output = ffmpeg.wait_with_output().unwrap();
    let status = output.status;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    debug!("FFmpeg process exited with status: {}", status);
    debug!("FFmpeg stdout: {}", stdout);
    debug!("FFmpeg stderr: {}", stderr);

    if !status.success() {
        error!("FFmpeg process failed with status: {}", status);
        error!("FFmpeg stderr: {}", stderr);
        return Err(anyhow::anyhow!(
            "FFmpeg process failed with status: {}",
            status
        ));
    }

    Ok(())
}

pub async fn encode_single_audio_async(
    data: Vec<f32>,
    sample_rate: u32,
    channels: u16,
    output_path: PathBuf,
) -> anyhow::Result<()> {
    if data.is_empty() {
        return Err(anyhow::anyhow!("No audio data provided for encoding"));
    }

    let ffmpeg_path = find_ffmpeg_path().ok_or_else(|| {
        anyhow::anyhow!("FFmpeg not found. Please install FFmpeg to save recordings.")
    })?;
    let output_name = output_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid checkpoint output path"))?;
    let temp_output_path = output_path.with_file_name(format!(".{}.finalizing", output_name));
    let _ = std::fs::remove_file(&temp_output_path);

    let mut command = tokio::process::Command::new(ffmpeg_path);
    command
        .arg("-f")
        .arg("f32le")
        .arg("-ar")
        .arg(sample_rate.to_string())
        .arg("-ac")
        .arg(channels.to_string())
        .arg("-i")
        .arg("pipe:0")
        .arg("-c:a")
        .arg("aac")
        .arg("-b:a")
        .arg("192k")
        .arg("-profile:a")
        .arg("aac_low")
        .arg("-movflags")
        .arg("+faststart")
        .arg("-f")
        .arg("mp4")
        .arg(&temp_output_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.as_std_mut().creation_flags(CREATE_NO_WINDOW);
    }

    let mut ffmpeg = command.spawn()?;
    let mut stdin = ffmpeg
        .stdin
        .take()
        .ok_or_else(|| anyhow::anyhow!("Failed to open FFmpeg stdin"))?;

    if let Err(write_error) = stdin.write_all(bytemuck::cast_slice(&data)).await {
        let _ = ffmpeg.kill().await;
        let _ = std::fs::remove_file(&temp_output_path);
        return Err(write_error.into());
    }
    drop(stdin);

    let output = ffmpeg.wait_with_output().await?;
    if !output.status.success() {
        let _ = std::fs::remove_file(&temp_output_path);
        return Err(anyhow::anyhow!(
            "FFmpeg process failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let output_metadata = std::fs::metadata(&temp_output_path)?;
    if output_metadata.len() == 0 {
        let _ = std::fs::remove_file(&temp_output_path);
        return Err(anyhow::anyhow!("FFmpeg produced an empty checkpoint"));
    }

    if output_path.exists() {
        std::fs::remove_file(&output_path)?;
    }
    std::fs::rename(&temp_output_path, &output_path)?;

    Ok(())
}
