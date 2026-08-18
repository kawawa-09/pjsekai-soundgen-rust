use std::collections::HashMap;
use std::io::Read;

use std::io::{Cursor, Write};
use std::process::{Command, Stdio};

use anyhow::{anyhow, Result};
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

#[derive(Debug, Clone)]
pub struct Sound {
    pub data: Vec<i16>,
    pub bitrate: u32,
}

impl Sound {
    pub fn load(buf: &[u8]) -> Sound {
        Sound::load_with_args(buf, &[])
    }
    pub fn load_with_args(buf: &[u8], args: &[String]) -> Sound {
        let mut child = Command::new("ffmpeg")
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
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let local_buf = buf.to_vec();
        let mut stdin = child.stdin.take().unwrap();
        let thread = std::thread::spawn(move || {
            stdin.write_all(&local_buf).unwrap();
        });
        let output = child.wait_with_output().unwrap();
        thread.join().unwrap();
        if !output.status.success() {
            panic!("ffmpeg failed");
        }
        let output_buf = output.stdout;
        Sound {
            data: output_buf.chunks_exact(2).map(|a| i16::from_le_bytes([a[0], a[1]])).collect(),
            bitrate: 48000,
        }
    }

    pub fn empty(bitrate: Option<u32>) -> Sound {
        Sound {
            data: vec![],
            bitrate: bitrate.unwrap_or(48000),
        }
    }

    fn sample_index(&self, seconds: f32) -> usize {
        (seconds * self.bitrate as f32) as usize * 2
    }

    fn overlay_samples(self, samples: impl Iterator<Item = i16>, start_index: usize, end_index: usize) -> Sound {
        let mut new_data = self.data;
        if end_index > new_data.len() {
            new_data.resize(end_index, 0);
        }
        let mixed = samples
            .zip(new_data[start_index..end_index].iter())
            .map(|(a, b)| a.saturating_add(*b))
            .collect::<Vec<i16>>();
        new_data.splice(start_index..end_index, mixed);

        Sound {
            data: new_data,
            bitrate: self.bitrate,
        }
    }

    pub fn overlay_at(self, other: &Sound, seconds: f32) -> Sound {
        let start_index = self.sample_index(seconds);
        let end_index = start_index + other.data.len();
        let samples = other.data.clone();
        self.overlay_samples(samples.into_iter(), start_index, end_index)
    }

    pub fn overlay_loop(self, other: &Sound, start: f32, end: f32) -> Sound {
        let start_index = self.sample_index(start);
        let end_index = self.sample_index(end);
        let samples = other.data.clone();
        self.overlay_samples(samples.into_iter().cycle(), start_index, end_index)
    }

    pub fn export(self, path: &str) {
        let mut child = Command::new("ffmpeg")
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
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        stdin
            .write_all(&self.data.iter().flat_map(|a| a.to_le_bytes().to_vec()).collect::<Vec<u8>>())
            .unwrap();
        drop(stdin);
        let output = child.wait_with_output().unwrap();
        if !output.status.success() {
            panic!("ffmpeg failed");
        }
    }

    pub fn overlay_until(self, sound: &Sound, start: f32, end: f32) -> Sound {
        let start_index = self.sample_index(start);
        let mut end_index = self.sample_index(end);
        if (end_index - start_index) > sound.data.len() {
            end_index = start_index + sound.data.len();
        }
        let samples = sound.data.clone();
        self.overlay_samples(
            samples.into_iter().take((end_index - start_index).saturating_sub(1)),
            start_index,
            end_index,
        )
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
            let mut file =
                zip.by_name(&clip.filename).map_err(|_| anyhow!("効果音のファイルが見つかりませんでした"))?;
            let mut buf = vec![];
            file.read_to_end(&mut buf).map_err(|_| anyhow!("効果音のファイルが読み込めませんでした"))?;
            audio.insert(clip.name, Sound::load(&buf));
        }
        Ok(Self { audio })
    }
}
