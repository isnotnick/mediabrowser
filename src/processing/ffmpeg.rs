use std::process::Command;
use std::path::{Path, PathBuf};
use anyhow::{Result, Context, anyhow};

pub fn find_ffmpeg() -> Result<PathBuf> {
    // Check local directory first
    if Path::new("./ffmpeg").exists() {
        return Ok(PathBuf::from("./ffmpeg"));
    }
    
    // Fallback to system PATH
    which::which("ffmpeg").context("ffmpeg not found in local directory or system PATH")
}

pub fn find_ffprobe() -> Result<PathBuf> {
    // Check local directory first
    if Path::new("./ffprobe").exists() {
        return Ok(PathBuf::from("./ffprobe"));
    }
    
    // Fallback to system PATH
    which::which("ffprobe").context("ffprobe not found in local directory or system PATH")
}

pub fn get_video_duration(path: &str) -> Result<f64> {
    let ffprobe_path = find_ffprobe()?;
    let output = Command::new(ffprobe_path)
        .args([
            "-v", "error",
            "-show_entries", "format=duration",
            "-of", "default=noprint_wrappers=1:nokey=1",
            path
        ])
        .output()?;
        
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("ffprobe failed: {}", err));
    }
    
    let duration_str = String::from_utf8_lossy(&output.stdout);
    let duration: f64 = duration_str.trim().parse()?;
    
    Ok(duration)
}

pub fn generate_thumbnail(
    input_path: &str,
    output_path: &str,
    is_video: bool,
    size: u32,
    quality: u8,
    duration: Option<f64>,
) -> Result<()> {
    let ffmpeg_path = find_ffmpeg()?;
    
    let scale_filter = format!("scale='min({},iw)':'min({},ih)':force_original_aspect_ratio=decrease", size, size);
    
    let mut cmd = Command::new(ffmpeg_path);
    
    if is_video {
        if let Some(dur) = duration {
            // Seek to 1% of video duration, fallback to 1 sec if duration is 0
            let seek_time = if dur > 0.0 { dur * 0.01 } else { 1.0 };
            cmd.args(["-ss", &format!("{:.3}", seek_time)]);
        } else {
            // Fallback to 1 second
            cmd.args(["-ss", "00:00:01"]);
        }
    }
    
    cmd.args([
        "-i", input_path,
        "-vframes", "1",
        "-vf", &scale_filter,
        "-q:v", &quality.to_string(), // Approximate translation of quality for mjpeg
        "-y", // overwrite
        output_path
    ]);
    
    let output = cmd.output()?;
    
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("ffmpeg thumbnail generation failed: {}", err));
    }

    Ok(())
}

pub fn generate_vtt_sprite_sheet(
    input_path: &str,
    output_image_path: &str,
    output_vtt_path: &str,
    duration: f64,
    size: u32,
) -> Result<()> {
    let ffmpeg_path = find_ffmpeg()?;
    
    // We'll generate an image every 5 seconds or max 100 images
    let interval = (duration / 100.0).max(5.0);
    
    // 1. Generate sprite sheet image
    // Using ffmpeg tile filter
    let columns = 10;
    // We need to calculate rows based on duration/interval
    let total_frames = (duration / interval).ceil() as u32;
    let rows = (total_frames as f32 / columns as f32).ceil() as u32;
    
    let filter = format!(
        "fps=1/{},scale={}:-1,tile={}x{}",
        interval, size, columns, rows
    );
    
    let output = Command::new(&ffmpeg_path)
        .args([
            "-i", input_path,
            "-vf", &filter,
            "-y",
            output_image_path
        ])
        .output()?;
        
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("ffmpeg sprite generation failed: {}", err));
    }
    
    // 2. Generate VTT file content
    let mut vtt_content = String::from("WEBVTT\n\n");
    
    let image_filename = Path::new(output_image_path).file_name().unwrap().to_string_lossy();
    
    // We assume scale={}:-1 keeps aspect ratio. 
    // For VTT we need the actual dimensions of each tile.
    // Let's assume standard 16:9 for tile height approximation if we don't probe again.
    let tile_width = size;
    let tile_height = (size as f32 * 9.0 / 16.0) as u32; // Rough approximation
    
    for i in 0..total_frames {
        let start_time = i as f64 * interval;
        let end_time = ((i + 1) as f64 * interval).min(duration);
        
        let start_fmt = format_time(start_time);
        let end_fmt = format_time(end_time);
        
        let col = i % columns;
        let row = i / columns;
        
        let x = col * tile_width;
        let y = row * tile_height;
        
        vtt_content.push_str(&format!("{} --> {}\n", start_fmt, end_fmt));
        vtt_content.push_str(&format!("{}#xywh={},{},{},{}\n\n", image_filename, x, y, tile_width, tile_height));
    }
    
    std::fs::write(output_vtt_path, vtt_content)?;
    
    Ok(())
}

fn format_time(seconds: f64) -> String {
    let hours = (seconds / 3600.0).floor() as u32;
    let mins = ((seconds % 3600.0) / 60.0).floor() as u32;
    let secs = (seconds % 60.0).floor() as u32;
    let millis = ((seconds.fract()) * 1000.0).floor() as u32;
    
    format!("{:02}:{:02}:{:02}.{:03}", hours, mins, secs, millis)
}
