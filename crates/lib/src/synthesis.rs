use crate::level::Level;
use crate::sound::SOUND_MAP;
use crate::sound::{Effect, Sound, LOOP_SOUND_MAP};
use crate::utils::debug;

use anyhow::{ensure, Result};
use itertools::Itertools;
use once_cell::sync::Lazy;
use std::sync;
use std::{collections::HashMap, thread};

#[derive(Debug, Clone)]
pub struct ClipColor {
    pub fg: &'static str,
    pub bg: &'static str,
}

// COLOR_MAP / NAME_MAP のキーは「正式名」(各候補リストの先頭要素) を使う。
static COLOR_MAP: Lazy<HashMap<&'static str, ClipColor>> = Lazy::new(|| {
    HashMap::from([
        ("#PERFECT", ClipColor { fg: "cyan", bg: "blue" }),
        (
            "#PERFECT_ALTERNATIVE",
            ClipColor {
                fg: "red",
                bg: "yellow",
            },
        ),
        (
            "#HOLD",
            ClipColor {
                fg: "green",
                bg: "blue",
            },
        ),
        (
            "Sekai Tick",
            ClipColor {
                fg: "green",
                bg: "blue",
            },
        ),
        (
            "Sekai Critical Tap",
            ClipColor {
                fg: "yellow",
                bg: "orange",
            },
        ),
        (
            "Sekai Critical Hold",
            ClipColor {
                fg: "yellow",
                bg: "orange",
            },
        ),
        (
            "Sekai Critical Flick",
            ClipColor {
                fg: "yellow",
                bg: "orange",
            },
        ),
        (
            "Sekai Critical Tick",
            ClipColor {
                fg: "yellow",
                bg: "orange",
            },
        ),
        (
            "Sekai Trace",
            ClipColor {
                fg: "green",
                bg: "blue",
            },
        ),
        (
            "Sekai Critical Trace",
            ClipColor {
                fg: "yellow",
                bg: "orange",
            },
        ),
    ])
});
static NAME_MAP: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    HashMap::from([
        ("#PERFECT", "通常タップ"),
        ("#PERFECT_ALTERNATIVE", "通常フリック"),
        ("#HOLD", "通常ホールド"),
        ("Sekai Tick", "スライド中継点"),
        ("Sekai Critical Tap", "金タップ"),
        ("Sekai Critical Hold", "金ホールド"),
        ("Sekai Critical Flick", "金フリック"),
        ("Sekai Critical Tick", "金スライド中継点"),
        ("Sekai Trace", "通常トレース"),
        ("Sekai Critical Trace", "金トレース"),
    ])
});

// 正式名(candidates[0]) -> 候補リスト の逆引きテーブル。
// SOUND_MAP / LOOP_SOUND_MAP の値(候補リスト)を先頭要素でまとめて引けるようにする。
static CANONICAL_MAP: Lazy<HashMap<&'static str, &'static [&'static str]>> = Lazy::new(|| {
    let mut map = HashMap::new();
    for candidates in SOUND_MAP.values().chain(LOOP_SOUND_MAP.values()) {
        map.insert(candidates[0], *candidates);
    }
    map
});

/// 正式名から、そのエンジンの effect リソースに実際に存在するクリップを探す。
/// 見つからなければ None（このSEはスキップする）。
fn resolve_sound(effect: &Effect, canonical_name: &str) -> Option<Sound> {
    let list: Vec<&str> = match CANONICAL_MAP.get(canonical_name).copied() {
        Some(candidates) => candidates.to_vec(),
        None => vec![canonical_name],
    };

    for name in list {
        if let Some(sound) = effect.audio.get(name) {
            if name != canonical_name {
                eprintln!("警告：SE「{}」が見つからないため「{}」で代用します。", canonical_name, name);
            }
            return Some(sound.clone());
        }
    }
    None
}

struct BpmChange {
    beat: f32,
    bpm: f32,
}

#[derive(Clone, Debug)]
pub struct Timing {
    single: HashMap<String, Vec<f32>>,
    connect: HashMap<String, Vec<(f32, f32)>>,
}

#[derive(Clone, Debug)]
pub struct ThreadInfo {
    pub color: ClipColor,
    pub max: i32,
}

#[derive(Clone, Debug)]
pub enum Progress {
    Info { threads: HashMap<String, ThreadInfo> },
    Update { id: String, current: i32 },
    Finish { id: String, sound: Sound },
}

pub async fn get_sound_timings(level: &Level, offset: f32) -> Result<Timing> {
    let mut timings: HashMap<String, Vec<f32>> = HashMap::new();
    let mut connect_timings: HashMap<String, Vec<(f32, f32)>> = HashMap::new();

    let mut bpm_changes: Vec<BpmChange> = vec![];
    for entity in level.data.entities.iter() {
        if entity.archetype == "#BPM_CHANGE" {
            bpm_changes.push(BpmChange {
                beat: entity
                    .get_value("#BEAT")
                    .ok_or_else(|| anyhow::anyhow!("譜面データが壊れています：#BPM_CHANGEに#BEATがありません"))?,
                bpm: entity
                    .get_value("#BPM")
                    .ok_or_else(|| anyhow::anyhow!("譜面データが壊れています：#BPM_CHANGEに#BPMがありません"))?,
            });
        }
    }
    bpm_changes.sort_by(|a, b| a.beat.partial_cmp(&b.beat).unwrap());
    let resolve_time = |beat: f32| -> f32 {
        let mut time = 0.0;
        let mut last_bpm = bpm_changes[0].bpm;
        let mut last_beat = 0.0;
        for bpm_change in bpm_changes.iter() {
            if bpm_change.beat > beat {
                break;
            }
            time += (bpm_change.beat - last_beat) * 60.0 / last_bpm;
            last_bpm = bpm_change.bpm;
            last_beat = bpm_change.beat;
        }
        time += (beat - last_beat) * 60.0 / last_bpm;
        time + level.data.bgm_offset + offset
    };
    for note in level.data.entities.iter() {
        let Some(candidates) = SOUND_MAP.get(&note.archetype.as_str()) else {
            continue;
        };
        // グルーピングのキーは候補リストの先頭要素（正式名）を使う
        let sound_data = candidates[0].to_string();
        if timings.get(&sound_data).is_none() {
            timings.insert(sound_data.clone(), vec![]);
        }
        let time = resolve_time(note.get_value("#BEAT").ok_or_else(|| {
            debug!(&note);
            anyhow::anyhow!("譜面データが壊れています：#BEATがありません")
        })?);
        timings.get_mut(&sound_data).unwrap().push(time);
    }
    let mut slide_connectors: HashMap<String, Vec<(f32, i32)>> = HashMap::new();
    for note in level.data.entities.iter() {
        // "Connector" は次RUSHエンジン等で使われる、Normal/Criticalの区別が
        // アーキタイプ名に含まれない汎用コネクター。見た目だけのガイド線など、
        // head/tailの構造が特殊なケースが混ざっている可能性があるため、
        // 取得に失敗した場合はプログラム全体を止めずにこのコネクターだけスキップする。
        if note.archetype == "Connector" {
            let Some(head) = note.get_ref(&level.data.entities, "head") else {
                eprintln!("[診断] Connector: headの参照先が見つからないためスキップします。");
                continue;
            };
            let Some(tail) = note.get_ref(&level.data.entities, "tail") else {
                eprintln!("[診断] Connector: tailの参照先が見つからないためスキップします。");
                continue;
            };
            let Some(head_beat) = head.get_value("#BEAT") else {
                eprintln!(
                    "[診断] Connector: headに#BEATが無いためスキップします。(head archetype={})",
                    head.archetype
                );
                continue;
            };
            let Some(tail_beat) = tail.get_value("#BEAT") else {
                eprintln!(
                    "[診断] Connector: tailに#BEATが無いためスキップします。(tail archetype={})",
                    tail.archetype
                );
                continue;
            };

            let is_critical =
                head.archetype.starts_with("Critical") || tail.archetype.starts_with("Critical");
            let key = if is_critical {
                "Sekai Critical Hold".to_string()
            } else {
                "#HOLD".to_string()
            };

            let head_time = resolve_time(head_beat);
            let tail_time = resolve_time(tail_beat);
            // 診断用：各Connectorがどの区間として登録されるかを出力
            eprintln!(
                "[診断] Connector key={} head_time={:.3} tail_time={:.3} head={} tail={}",
                key, head_time, tail_time, head.archetype, tail.archetype
            );
            if slide_connectors.get(&key).is_none() {
                slide_connectors.insert(key.clone(), vec![]);
            }
            slide_connectors.get_mut(&key).unwrap().push((head_time, 1));
            slide_connectors.get_mut(&key).unwrap().push((tail_time, -1));
            continue;
        }

        let Some(candidates) = LOOP_SOUND_MAP.get(&note.archetype.as_str()) else {
            continue;
        };
        let key = candidates[0].to_string();

        let head = note
            .get_ref(&level.data.entities, "head")
            .ok_or_else(|| anyhow::anyhow!("譜面データが壊れています：SlideConnectorにheadがありません"))?;
        let tail = note
            .get_ref(&level.data.entities, "tail")
            .ok_or_else(|| anyhow::anyhow!("譜面データが壊れています：SlideConnectorにtailがありません"))?;
        let head_time = resolve_time(
            head.get_value("#BEAT")
                .ok_or_else(|| anyhow::anyhow!("譜面データが壊れています：SlideConnectorのheadに#BEATがありません"))?,
        );
        let tail_time = resolve_time(
            tail.get_value("#BEAT")
                .ok_or_else(|| anyhow::anyhow!("譜面データが壊れています：SlideConnectorのtailに#BEATがありません"))?,
        );
        if slide_connectors.get(&key).is_none() {
            slide_connectors.insert(key.clone(), vec![]);
        }
        slide_connectors.get_mut(&key).unwrap().push((head_time, 1));
        slide_connectors.get_mut(&key).unwrap().push((tail_time, -1));
    }
    for (key, changes) in slide_connectors.iter() {
        let mut slide_count = 0;
        let mut grouped_changes = changes
            .iter()
            .group_by(|(time, _)| *time)
            .into_iter()
            .map(|(time, changes)| (time, changes.map(|(_, change)| *change).collect::<Vec<_>>()))
            .collect::<Vec<_>>();

        grouped_changes.sort_by(|(time1, _), (time2, _)| time1.partial_cmp(time2).unwrap());

        for (time, changes) in &grouped_changes {
            if connect_timings.get(key).is_none() {
                connect_timings.insert(key.clone(), vec![]);
            }
            let time = *time;
            let change = changes.iter().sum::<i32>();
            if change == 0 {
                continue;
            }
            slide_count += change;
            let timing = connect_timings.get_mut(key).unwrap();
            if timing.is_empty() {
                timing.push((time, -1.0));
            } else if slide_count == 0 && change < 0 {
                timing.last_mut().unwrap().1 = time;
            } else if slide_count == 1 && change > 0 {
                timing.push((time, -1.0));
            }
            ensure!(slide_count >= 0, "譜面データが壊れています：スライドの開始と終了の数が一致しません");
        }
        ensure!(slide_count == 0, "譜面データが壊れています：スライドの開始と終了の数が一致しません");
        ensure!(
            connect_timings.get(key).unwrap().last().unwrap().1 != -1.0,
            "譜面データが壊れています：スライドの開始と終了の数が一致しません"
        );
    }
    timings.values_mut().for_each(|v| {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v.dedup()
    });

    Ok(Timing {
        single: timings,
        connect: connect_timings,
    })
}

pub async fn synthesis(timing: &Timing, effect: &Effect, notes_per_thread: usize) -> sync::mpsc::Receiver<Progress> {
    let (tx, rx) = sync::mpsc::channel::<Progress>();
    let timing = timing.clone();
    let effect = effect.clone();

    thread::spawn(move || {
        let mut thread_infos: HashMap<String, ThreadInfo> = HashMap::new();
        let mut threads: Vec<thread::JoinHandle<()>> = vec![];
        for (sound_name, timings) in timing.single.iter() {
            // 正式名(sound_name)に対応する実際の効果音クリップを候補リストから解決する。
            // どの候補も effect.audio に存在しない場合はこのSEをスキップ（panicしない）。
            let Some(sound) = resolve_sound(&effect, sound_name) else {
                eprintln!(
                    "警告：不明なSEです：{}。このSEはスキップされます。Issueに報告してください。",
                    sound_name
                );
                continue;
            };
            let color = COLOR_MAP
                .get(sound_name.as_str())
                .cloned()
                .unwrap_or(ClipColor { fg: "gray", bg: "gray" });
            let name = NAME_MAP.get(sound_name.as_str()).copied().unwrap_or(sound_name.as_str());

            let thread_count = (timings.len() + notes_per_thread - 1) / notes_per_thread;
            let notes_per_thread = (timings.len() + thread_count - 1) / thread_count;
            for i in 0..thread_count {
                let start = i * notes_per_thread;
                let end = if i == thread_count - 1 {
                    timings.len()
                } else {
                    std::cmp::min((i + 1) * notes_per_thread, timings.len())
                };
                let timings = timings[start..end].to_vec();
                let tx = tx.clone();
                debug!(&sound_name);
                let sound = sound.clone();

                let id = format!("{} ({})", name, i + 1);

                thread_infos.insert(
                    id.clone(),
                    ThreadInfo {
                        color: color.clone(),
                        max: timings.len() as i32,
                    },
                );
                threads.push(thread::spawn(move || {
                    thread::park();
                    let mut local_sound = Sound::empty(None);
                    for (i, time) in timings.iter().enumerate() {
                        let next_time = timings.get(i + 1).unwrap_or(&(*time + 5.0)).to_owned();
                        local_sound = local_sound.overlay_until(&sound, *time, next_time);
                        tx.send(Progress::Update {
                            id: id.clone(),
                            current: i as i32 + 1,
                        })
                        .unwrap();
                    }
                    tx.send(Progress::Finish { id, sound: local_sound }).unwrap();
                }));
            }
        }
        for (sound_name, timings) in timing.connect.iter() {
            let Some(sound) = resolve_sound(&effect, sound_name) else {
                eprintln!(
                    "警告：不明なSEです：{}。このSEはスキップされます。Issueに報告してください。",
                    sound_name
                );
                continue;
            };
            let color = COLOR_MAP
                .get(sound_name.as_str())
                .cloned()
                .unwrap_or(ClipColor { fg: "gray", bg: "gray" });
            let name = NAME_MAP.get(sound_name.as_str()).copied().unwrap_or(sound_name.as_str());

            let timings = timings.clone();
            let tx = tx.clone();

            let id = name.to_string();

            thread_infos.insert(
                id.clone(),
                ThreadInfo {
                    color: color.clone(),
                    max: timings.len() as i32,
                },
            );
            threads.push(thread::spawn(move || {
                thread::park();
                let mut local_sound = Sound::empty(None);
                for (i, (start, end)) in timings.iter().enumerate() {
                    local_sound = local_sound.overlay_loop(&sound, start.to_owned(), end.to_owned());
                    tx.send(Progress::Update {
                        id: id.clone(),
                        current: i as i32 + 1,
                    })
                    .unwrap();
                }
                tx.send(Progress::Finish { id, sound: local_sound }).unwrap();
            }));
        }
        tx.send(Progress::Info { threads: thread_infos }).unwrap();
        for thread in threads {
            thread.thread().unpark();
        }
    });

    rx
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::Server;
    use crate::sonolus::{LevelData, LevelInfo};

    fn test_level(bgm_offset: f32, entities: serde_json::Value) -> Level {
        let server = Server::guess("frpt-test").unwrap();
        let info: LevelInfo = serde_json::from_value(serde_json::json!({
            "title": "Test",
            "artists": "Artist",
            "author": "Author",
            "name": "frpt-test",
            "rating": 1,
            "bgm": {"url": "/bgm.mp3"},
            "data": {"url": "/data.gz"},
            "engine": {
                "version": 1,
                "effect": {
                    "audio": {"url": "/audio.zip"},
                    "data": {"url": "/effect.gz"},
                },
            },
        }))
        .unwrap();
        let data: LevelData = serde_json::from_value(serde_json::json!({
            "bgmOffset": bgm_offset,
            "entities": entities,
        }))
        .unwrap();
        Level::new(server, info, data)
    }

    fn bpm_change(beat: f32, bpm: f32) -> serde_json::Value {
        serde_json::json!({
            "archetype": "#BPM_CHANGE",
            "data": [{"name": "#BEAT", "value": beat}, {"name": "#BPM", "value": bpm}],
            "name": null,
        })
    }

    fn tap(archetype: &str, beat: f32) -> serde_json::Value {
        serde_json::json!({
            "archetype": archetype,
            "data": [{"name": "#BEAT", "value": beat}],
            "name": null,
        })
    }

    fn named_tap(archetype: &str, beat: f32, name: &str) -> serde_json::Value {
        serde_json::json!({
            "archetype": archetype,
            "data": [{"name": "#BEAT", "value": beat}],
            "name": name,
        })
    }

    fn connector(archetype: &str, head: &str, tail: &str) -> serde_json::Value {
        serde_json::json!({
            "archetype": archetype,
            "data": [{"name": "head", "ref": head}, {"name": "tail", "ref": tail}],
            "name": null,
        })
    }

    fn test_effect(clips: &[(&str, usize)]) -> Effect {
        let mut audio = std::collections::HashMap::new();
        for (name, samples) in clips {
            audio.insert(
                name.to_string(),
                Sound {
                    data: vec![1; *samples],
                    bitrate: 48000,
                },
            );
        }
        Effect { audio }
    }

    #[test]
    fn resolve_sound_finds_exact_clip() {
        let effect = test_effect(&[("#PERFECT", 4)]);
        assert!(resolve_sound(&effect, "#PERFECT").is_some());
    }

    #[test]
    fn resolve_sound_falls_back_to_alternative_candidates() {
        let effect = test_effect(&[("Sekai Normal Trace", 4)]);
        assert!(resolve_sound(&effect, "Sekai Trace").is_some());
    }

    #[test]
    fn resolve_sound_returns_none_for_missing_clip() {
        let effect = test_effect(&[("#PERFECT", 4)]);
        assert!(resolve_sound(&effect, "Sekai Critical Tap").is_none());
        assert!(resolve_sound(&effect, "unknown clip").is_none());
    }

    #[tokio::test]
    async fn get_sound_timings_resolves_note_times_at_constant_bpm() {
        let level = test_level(0.0, serde_json::json!([bpm_change(0.0, 60.0), tap("NormalTapNote", 2.0)]));
        let timing = get_sound_timings(&level, 0.0).await.unwrap();
        assert_eq!(timing.single.get("#PERFECT"), Some(&vec![2.0]));
        assert!(timing.connect.is_empty());
    }

    #[tokio::test]
    async fn get_sound_timings_applies_bgm_offset_and_shift() {
        let level = test_level(0.5, serde_json::json!([bpm_change(0.0, 60.0), tap("NormalTapNote", 1.0)]));
        let timing = get_sound_timings(&level, 0.25).await.unwrap();
        assert_eq!(timing.single.get("#PERFECT"), Some(&vec![1.75]));
    }

    #[tokio::test]
    async fn get_sound_timings_handles_bpm_changes() {
        let level = test_level(
            0.0,
            serde_json::json!([bpm_change(0.0, 60.0), bpm_change(2.0, 120.0), tap("NormalTapNote", 4.0)]),
        );
        let timing = get_sound_timings(&level, 0.0).await.unwrap();
        // 2 beats at 60 BPM (2s) + 2 beats at 120 BPM (1s)
        assert_eq!(timing.single.get("#PERFECT"), Some(&vec![3.0]));
    }

    #[tokio::test]
    async fn get_sound_timings_dedupes_and_sorts_timings() {
        let level = test_level(
            0.0,
            serde_json::json!([
                bpm_change(0.0, 60.0),
                tap("NormalTapNote", 3.0),
                tap("NormalTapNote", 1.0),
                tap("NormalSlideStartNote", 1.0),
            ]),
        );
        let timing = get_sound_timings(&level, 0.0).await.unwrap();
        assert_eq!(timing.single.get("#PERFECT"), Some(&vec![1.0, 3.0]));
    }

    #[tokio::test]
    async fn get_sound_timings_groups_notes_by_canonical_clip_name() {
        let level = test_level(
            0.0,
            serde_json::json!([
                bpm_change(0.0, 60.0),
                tap("NormalTapNote", 1.0),
                tap("NormalFlickNote", 2.0),
                tap("CriticalTapNote", 3.0),
                tap("UnknownArchetype", 4.0),
            ]),
        );
        let timing = get_sound_timings(&level, 0.0).await.unwrap();
        assert_eq!(timing.single.get("#PERFECT"), Some(&vec![1.0]));
        assert_eq!(timing.single.get("#PERFECT_ALTERNATIVE"), Some(&vec![2.0]));
        assert_eq!(timing.single.get("Sekai Critical Tap"), Some(&vec![3.0]));
        assert_eq!(timing.single.len(), 3);
    }

    #[tokio::test]
    async fn get_sound_timings_merges_overlapping_slide_connectors() {
        let level = test_level(
            0.0,
            serde_json::json!([
                bpm_change(0.0, 60.0),
                named_tap("NormalSlideStartNote", 1.0, "a"),
                named_tap("NormalSlideEndNote", 3.0, "b"),
                named_tap("NormalSlideStartNote", 2.0, "c"),
                named_tap("NormalSlideEndNote", 4.0, "d"),
                connector("NormalSlideConnector", "a", "b"),
                connector("NormalSlideConnector", "c", "d"),
            ]),
        );
        let timing = get_sound_timings(&level, 0.0).await.unwrap();
        assert_eq!(timing.connect.get("#HOLD"), Some(&vec![(1.0, 4.0)]));
    }

    #[tokio::test]
    async fn get_sound_timings_registers_generic_connector_as_critical_hold() {
        let level = test_level(
            0.0,
            serde_json::json!([
                bpm_change(0.0, 60.0),
                named_tap("CriticalSlideStartNote", 1.0, "a"),
                named_tap("CriticalSlideEndNote", 2.0, "b"),
                connector("Connector", "a", "b"),
            ]),
        );
        let timing = get_sound_timings(&level, 0.0).await.unwrap();
        assert_eq!(timing.connect.get("Sekai Critical Hold"), Some(&vec![(1.0, 2.0)]));
    }

    #[tokio::test]
    async fn get_sound_timings_skips_generic_connector_with_broken_refs() {
        let level = test_level(
            0.0,
            serde_json::json!([bpm_change(0.0, 60.0), connector("Connector", "missing", "also-missing")]),
        );
        let timing = get_sound_timings(&level, 0.0).await.unwrap();
        assert!(timing.connect.is_empty());
    }

    #[tokio::test]
    async fn get_sound_timings_fails_on_bpm_change_without_bpm() {
        let level = test_level(
            0.0,
            serde_json::json!([{
                "archetype": "#BPM_CHANGE",
                "data": [{"name": "#BEAT", "value": 0.0}],
                "name": null,
            }]),
        );
        assert!(get_sound_timings(&level, 0.0).await.is_err());
    }

    #[tokio::test]
    async fn get_sound_timings_fails_on_note_without_beat() {
        let level = test_level(
            0.0,
            serde_json::json!([bpm_change(0.0, 60.0), {
                "archetype": "NormalTapNote",
                "data": [],
                "name": null,
            }]),
        );
        assert!(get_sound_timings(&level, 0.0).await.is_err());
    }

    #[tokio::test]
    async fn get_sound_timings_fails_on_unbalanced_slide_connector() {
        let level = test_level(
            0.0,
            serde_json::json!([
                bpm_change(0.0, 60.0),
                named_tap("NormalSlideStartNote", 1.0, "a"),
                connector("NormalSlideConnector", "a", "missing"),
            ]),
        );
        assert!(get_sound_timings(&level, 0.0).await.is_err());
    }

    #[tokio::test]
    async fn synthesis_reports_progress_and_finishes() {
        let level = test_level(
            0.0,
            serde_json::json!([
                bpm_change(0.0, 60.0),
                tap("NormalTapNote", 0.0),
                tap("NormalTapNote", 1.0),
                named_tap("NormalSlideStartNote", 1.0, "a"),
                named_tap("NormalSlideEndNote", 2.0, "b"),
                connector("NormalSlideConnector", "a", "b"),
            ]),
        );
        let timing = get_sound_timings(&level, 0.0).await.unwrap();
        let effect = test_effect(&[("#PERFECT", 4), ("#HOLD", 4)]);

        let rx = synthesis(&timing, &effect, 1000).await;
        let Progress::Info { threads } = rx.recv().unwrap() else {
            panic!("first message should be Progress::Info");
        };
        assert!(!threads.is_empty());

        let mut finished = 0;
        let mut updates = 0;
        while finished < threads.len() {
            match rx.recv().unwrap() {
                Progress::Update { id, .. } => {
                    assert!(threads.contains_key(&id));
                    updates += 1;
                }
                Progress::Finish { id, sound } => {
                    assert!(threads.contains_key(&id));
                    assert!(!sound.data.is_empty());
                    finished += 1;
                }
                Progress::Info { .. } => panic!("Progress::Info should only be sent once"),
            }
        }
        assert!(updates > 0);
    }

    #[tokio::test]
    async fn synthesis_skips_unknown_sounds() {
        let level = test_level(0.0, serde_json::json!([bpm_change(0.0, 60.0), tap("NormalTapNote", 1.0)]));
        let timing = get_sound_timings(&level, 0.0).await.unwrap();
        let effect = test_effect(&[]);

        let rx = synthesis(&timing, &effect, 1000).await;
        let Progress::Info { threads } = rx.recv().unwrap() else {
            panic!("first message should be Progress::Info");
        };
        assert!(threads.is_empty());
    }
}
