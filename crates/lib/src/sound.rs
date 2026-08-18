use std::collections::HashMap;
use std::io::Read;

use std::io::{Cursor, Write};
use std::process::{Child, Command, ExitStatus, Stdio};

use anyhow::{anyhow, Context, Result};
use once_cell::sync::Lazy;
use zip::ZipArchive;

use crate::sonolus::EffectData;

pub static SOUND_MAP: Lazy<HashMap<&'static str, &'static [&'static str]>> = Lazy::new(|| {
    HashMap::from([
        ("NormalTapNote", &["#PERFECT"][..]),
        ("CriticalTapNote", &["Sekai Critical Tap"][..]),
        ("NormalFlickNote", &["#PERFECT_ALTERNATIVE"][..]),
        ("CriticalFlickNote", &["Sekai Critical Flick"][..]),
        ("NormalSlideStartNote", &["#PERFECT"][..]),
        ("NormalHeadTapNote", &["#PERFECT"][..]),
        ("CriticalSlideStartNote", &["#PERFECT"][..]),
        ("CriticalHeadTapNote", &["#PERFECT"][..]),
        ("NormalSlideEndNote", &["#PERFECT"][..]),
        ("NormalTailReleaseNote", &["#PERFECT"][..]),
        ("CriticalSlideEndNote", &["#PERFECT"][..]),
        ("CriticalTailReleaseNote", &["#PERFECT"][..]),
        ("NormalSlideEndFlickNote", &["#PERFECT_ALTERNATIVE"][..]),
        ("NormalTailFlickNote", &["#PERFECT_ALTERNATIVE"][..]),
        ("CriticalSlideEndFlickNote", &["Sekai Critical Flick"][..]),
        ("CriticalTailFlickNote", &["Sekai Critical Flick"][..]),
        ("NormalTickNote", &["Sekai Tick"][..]),
        ("CriticalTickNote", &["Sekai Critical Tick"][..]),
        ("NormalAttachedSlideTickNote", &["Sekai Tick"][..]),
        ("CriticalAttachedSlideTickNote", &["Sekai Critical Tick"][..]),
        ("NormalTraceNote", &["Sekai Trace", "Sekai Normal Trace"][..]),
        ("CriticalTraceNote", &["Sekai Critical Trace"][..]),
        ("NormalTraceFlickNote", &["#PERFECT_ALTERNATIVE"][..]),
        ("NormalTailTraceFlickNote", &["#PERFECT_ALTERNATIVE"][..]),
        ("CriticalTraceFlickNote", &["Sekai Critical Flick"][..]),
        ("CriticalTailTraceFlickNote", &["Sekai Critical Flick"][..]),
        ("NonDirectionalTraceFlickNote", &["#PERFECT_ALTERNATIVE"][..]),
        ("NormalTraceSlideStartNote", &["Sekai Trace", "Sekai Normal Trace"][..]),
        ("NormalHeadTraceNote", &["Sekai Trace", "Sekai Normal Trace"][..]),
        ("CriticalTraceSlideStartNote", &["Sekai Critical Trace"][..]),
        ("CriticalHeadTraceNote", &["Sekai Critical Trace"][..]),
        ("NormalTraceSlideEndNote", &["Sekai Trace", "Sekai Normal Trace"][..]),
        ("NormalTailTraceNote", &["Sekai Trace", "Sekai Normal Trace"][..]),
        ("CriticalTraceSlideEndNote", &["Sekai Critical Trace"][..]),
        ("CriticalTailTraceNote", &["Sekai Critical Trace"][..]),
    ])
});
pub static LOOP_SOUND_MAP: Lazy<HashMap<&'static str, &'static [&'static str]>> = Lazy::new(|| {
    HashMap::from([
        ("NormalSlideConnector", &["#HOLD"][..]),
        ("NormalConnector", &["#HOLD"][..]),
        ("CriticalSlideConnector", &["Sekai Critical Hold"][..]),
        ("CriticalConnector", &["Sekai Critical Hold"][..]),
    ])
});

fn spawn_ffmpeg(command: &mut Command) -> Result<Child> {
    command.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            anyhow!("ffmpegが見つかりませんでした。ffmpegをインストールし、PATHに追加してください。")
        } else {
            anyhow!("ffmpegの起動に失敗しました。: {}", e)
        }
    })
}

fn ffmpeg_error(message: &str, status: ExitStatus, stderr: &[u8]) -> anyhow::Error {
    let stderr = String::from_utf8_lossy(stderr);
    let mut last_lines = stderr.lines().rev().take(5).collect::<Vec<_>>();
    last_lines.reverse();
    let detail = last_lines.join("\n");
    if detail.is_empty() {
        anyhow!("{}（ffmpegの終了コード: {}）", message, status)
    } else {
        anyhow!("{}（ffmpegの終了コード: {}）\n{}", message, status, detail)
    }
}

/// ffmpegが入力を受け取る前に終了した場合、書き込みはBrokenPipeで失敗する。
/// その場合の本当の原因はffmpegの終了ステータスとstderrに現れるため、ここでは無視する。
fn write_ignoring_broken_pipe(writer: &mut impl Write, buf: &[u8]) -> Result<()> {
    match writer.write_all(buf) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        Err(e) => Err(anyhow!("ffmpegへのデータの書き込みに失敗しました。: {}", e)),
    }
}

#[derive(Debug, Clone)]
pub struct Sound {
    pub data: Vec<i16>,
    pub bitrate: u32,
}

impl Sound {
    pub fn load(buf: &[u8]) -> Result<Sound> {
        Sound::load_with_args(buf, &[])
    }
    pub fn load_with_args(buf: &[u8], args: &[String]) -> Result<Sound> {
        let mut child = spawn_ffmpeg(
            Command::new("ffmpeg")
                .arg("-i")
                .arg("-")
                .args(args)
                .arg("-ac")
                .arg("2")
                .arg("-f")
                .arg("s16le")
                .arg("-ar")
                .arg("48k")
                .arg("-")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped()),
        )?;
        let local_buf = buf.to_vec();
        let mut stdin = child.stdin.take().context("ffmpegの標準入力を取得できませんでした。")?;
        let thread = std::thread::spawn(move || write_ignoring_broken_pipe(&mut stdin, &local_buf));
        let output = child.wait_with_output().map_err(|e| anyhow!("ffmpegの実行に失敗しました。: {}", e))?;
        let write_result = thread.join().map_err(|_| anyhow!("ffmpegへの書き込みスレッドが異常終了しました。"))?;
        if !output.status.success() {
            return Err(ffmpeg_error("音声の読み込みに失敗しました。", output.status, &output.stderr));
        }
        write_result?;
        let output_buf = output.stdout;
        Ok(Sound {
            data: output_buf.chunks_exact(2).map(|a| i16::from_le_bytes([a[0], a[1]])).collect(),
            bitrate: 48000,
        })
    }

    pub fn empty(bitrate: Option<u32>) -> Sound {
        Sound {
            data: vec![],
            bitrate: bitrate.unwrap_or(48000),
        }
    }

    pub fn overlay_at(self, other: &Sound, seconds: f32) -> Sound {
        let mut new_data = self.data.clone();
        let start_index = (seconds * self.bitrate as f32) as usize * 2;
        let end_index = start_index + other.data.len();
        if end_index > new_data.len() {
            new_data.resize(end_index, 0);
        }
        new_data.splice(
            start_index..end_index,
            other
                .data
                .iter()
                .cloned()
                .zip(new_data.clone()[start_index..end_index].iter())
                .map(|(a, b)| a.saturating_add(*b))
                .collect::<Vec<i16>>(),
        );

        Sound {
            data: new_data,
            bitrate: self.bitrate,
        }
    }

    pub fn overlay_loop(self, other: &Sound, start: f32, end: f32) -> Sound {
        let mut new_data = self.data.clone();
        let start_index = (start * self.bitrate as f32) as usize * 2;
        let end_index = (end * self.bitrate as f32) as usize * 2;
        if end_index > new_data.len() {
            new_data.resize(end_index, 0);
        }
        new_data.splice(
            start_index..end_index,
            other
                .data
                .iter()
                .cycle()
                .cloned()
                .zip(new_data.clone()[start_index..end_index].iter())
                .map(|(a, b)| a.saturating_add(*b))
                .collect::<Vec<i16>>(),
        );

        Sound {
            data: new_data,
            bitrate: self.bitrate,
        }
    }

    pub fn export(self, path: &str) -> Result<()> {
        let mut child = spawn_ffmpeg(
            Command::new("ffmpeg")
                .arg("-y")
                .args(["-f", "s16le"])
                .args(["-c:a", "pcm_s16le"])
                .args(["-ar", self.bitrate.to_string().as_str()])
                .args(["-ac", "2"])
                .args(["-i", "-"])
                .args(["-b:a", "480k"])
                .args(["-maxrate", "480k"])
                .args(["-bufsize", "480k"])
                .args(["-minrate", "480k"])
                .arg(path)
                .stdin(Stdio::piped())
                .stderr(Stdio::piped()),
        )?;
        let mut stdin = child.stdin.take().context("ffmpegの標準入力を取得できませんでした。")?;
        let write_result = write_ignoring_broken_pipe(
            &mut stdin,
            &self.data.iter().flat_map(|a| a.to_le_bytes().to_vec()).collect::<Vec<u8>>(),
        );
        drop(stdin);
        let output = child.wait_with_output().map_err(|e| anyhow!("ffmpegの実行に失敗しました。: {}", e))?;
        if !output.status.success() {
            return Err(ffmpeg_error("音声の書き出しに失敗しました。", output.status, &output.stderr));
        }
        write_result
    }

    pub fn overlay_until(self, sound: &Sound, start: f32, end: f32) -> Sound {
        let mut new_data = self.data.clone();
        let start_index = (start * self.bitrate as f32) as usize * 2;
        let mut end_index = (end * self.bitrate as f32) as usize * 2;
        if (end_index - start_index) > sound.data.len() {
            end_index = start_index + sound.data.len();
        }
        if end_index > new_data.len() {
            new_data.resize(end_index, 0);
        }
        new_data.splice(
            start_index..end_index,
            sound
                .data
                .iter()
                .cloned()
                .zip(new_data.clone()[start_index..end_index - 1].iter())
                .map(|(a, b)| a.saturating_add(*b))
                .collect::<Vec<i16>>(),
        );

        Sound {
            data: new_data,
            bitrate: self.bitrate,
        }
    }
}

impl std::ops::Mul<f32> for Sound {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self {
        let mut result = vec![];
        for a in self.data.iter() {
            result.push(((*a as f32) * rhs) as i16);
        }
        Sound {
            data: result,
            bitrate: self.bitrate,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Effect {
    pub audio: HashMap<String, Sound>,
}

impl Effect {
    pub fn new(data: EffectData, mut zip: ZipArchive<Cursor<Vec<u8>>>) -> Result<Self> {
        let mut audio = HashMap::new();
        for clip in data.clips {
            let mut file = zip
                .by_name(&clip.filename)
                .map_err(|e| anyhow!("効果音のファイルが見つかりませんでした（{}）: {}", clip.filename, e))?;
            let mut buf = vec![];
            file.read_to_end(&mut buf)
                .map_err(|e| anyhow!("効果音のファイルが読み込めませんでした（{}）: {}", clip.filename, e))?;
            let sound =
                Sound::load(&buf).map_err(|e| anyhow!("効果音「{}」の読み込みに失敗しました。: {}", clip.name, e))?;
            audio.insert(clip.name, sound);
        }
        Ok(Self { audio })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_returns_error_for_invalid_audio() {
        let error = Sound::load(b"not an audio file").unwrap_err().to_string();
        assert!(error.contains("音声の読み込みに失敗しました。"), "unexpected error: {}", error);
    }

    #[test]
    fn export_returns_error_for_invalid_path() {
        let error = Sound::empty(None).export("./no_such_directory/out.mp3").unwrap_err().to_string();
        assert!(error.contains("音声の書き出しに失敗しました。"), "unexpected error: {}", error);
    }
}
