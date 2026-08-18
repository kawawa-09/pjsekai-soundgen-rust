mod console;
mod utils;

use crate::{console::show_title, utils::rgb};
use anyhow::{anyhow, bail, Context, Result};
use dialoguer::{theme::ColorfulTheme, Input};
use getopts::Options;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use octocrab::Octocrab;
use pjsekai_soundgen_core::{server::Server, sound::Sound, synthesis::Progress};
use std::{
    collections::HashMap,
    io::ErrorKind,
    path::{Path, PathBuf},
    thread, {env, fs},
};
use tokio::{
    fs::File,
    io::{AsyncReadExt, AsyncWriteExt},
};

static LOG_STYLE: &str = "[{elapsed_precise} / {eta_precise}] [{bar:50.{color_fg}/{color_bg}}] {pos:>7}/{len:7} {msg}";

struct Args {
    bgm_override: Option<String>,
    bgm_volume: f32,
    shift: f32,
    silent: bool,
    output: Option<String>,
    id: Option<String>,
    notes_per_thread: usize,
}

fn parse_opt<T: std::str::FromStr>(matches: &getopts::Matches, name: &str, default: T) -> Result<T> {
    match matches.opt_str(name) {
        Some(value) => value.parse::<T>().map_err(|_| anyhow!("--{}に指定された値が不正です：{}", name, value)),
        None => Ok(default),
    }
}

fn parse_args() -> Result<Args> {
    let mut opts = Options::new();
    opts.optflag("h", "help", "ヘルプを表示して終了します。");
    opts.optopt("b", "bgm", "BGMを上書きします。", "PATH");
    opts.optopt("v", "bgm-volume", "BGMのボリュームを指定します。（1.0で等倍）", "VOLUME");
    opts.optopt("s", "shift", "SEをずらします。（秒単位）", "SECONDS");
    opts.optflag("S", "silent", "SEのみを生成します。");
    opts.optopt("n", "notes-per-thread", "スレッド毎のノーツ数を指定します。", "NUMBER");
    opts.optopt("o", "output", "出力先を指定します。", "OUTPUT");
    let matches = match opts.parse(env::args().collect::<Vec<_>>()) {
        Ok(m) => m,
        Err(f) => {
            println!("{}", f);
            println!("{}", opts.usage(""));
            std::process::exit(1);
        }
    };
    if matches.opt_present("h") {
        let args: Vec<String> = env::args().collect();
        println!("{}", opts.usage(format!("{} [OPTIONS] [ID]", &args[0]).as_str()));
        std::process::exit(0);
    }
    let notes_per_thread = parse_opt::<usize>(&matches, "n", 1000)?;
    if notes_per_thread == 0 {
        bail!("--nには1以上の値を指定してください。");
    }
    Ok(Args {
        bgm_override: matches.opt_str("b"),
        bgm_volume: parse_opt::<f32>(&matches, "v", 1.0)?,
        shift: parse_opt::<f32>(&matches, "s", 0.0)?,
        silent: matches.opt_present("S"),
        output: matches.opt_str("o"),
        id: matches.free.get(1).map(|s| s.to_string()),
        notes_per_thread,
    })
}

fn update_check_flag_path() -> Option<PathBuf> {
    let executable_path = process_path::get_executable_path()?;
    Some(executable_path.parent()?.join(".update-check"))
}

/// 前回の更新確認から1日以上経っているかを返す。
/// フラグファイルが読めない・壊れている場合は、確認し直せばよいのでtrueを返す。
fn should_check_update(flag_path: &Path) -> bool {
    let Ok(flag) = fs::read_to_string(flag_path) else {
        return true;
    };
    let Ok(last_checked) = chrono::DateTime::parse_from_rfc3339(flag.trim()) else {
        return true;
    };
    chrono::Local::now().signed_duration_since(last_checked).num_days() >= 1
}

async fn check_update(flag_path: &Path) -> Result<()> {
    let mut file = File::create(flag_path)
        .await
        .with_context(|| format!("更新確認の記録ファイルを作成できませんでした。（{}）", flag_path.display()))?;
    let now = chrono::Local::now();
    file.write_all(now.to_rfc3339().as_bytes()).await.context("更新確認の記録に失敗しました。")?;
    let octocrab = Octocrab::builder().build().context("GitHubクライアントの初期化に失敗しました。")?;
    let release = octocrab
        .repos("sevenc-nanashi", "pjsekai-soundgen-rust")
        .releases()
        .get_latest()
        .await
        .context("最新バージョンの取得に失敗しました。")?;
    let version = release.tag_name.trim_start_matches('v');
    let current_version = env!("CARGO_PKG_VERSION");
    if version != current_version {
        console::info(&format!("新しいバージョンがリリースされています：v{} -> v{}", current_version, version));
        console::info(&format!("ダウンロード：{}", release.html_url));
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    let ansi = enable_ansi_support::enable_ansi_support().is_ok();
    console::ANSI.store(ansi, std::sync::atomic::Ordering::SeqCst);
    show_title();
    if let Err(err) = run().await {
        console::error(&format!("{:#}", err));
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    // 更新確認は本来の処理ではないため、失敗しても警告のみで続行する。
    if let Some(flag_path) = update_check_flag_path() {
        if should_check_update(&flag_path) {
            if let Err(err) = check_update(&flag_path).await {
                console::warning(&format!("更新の確認に失敗しました。: {:#}", err));
            }
        }
    }
    let args = parse_args()?;
    if args.output.is_none() {
        if let Err(err) = fs::create_dir("./dist") {
            if err.kind() != ErrorKind::AlreadyExists {
                return Err(anyhow!("distフォルダを作成できませんでした。: {}", err));
            }
        }
    }
    let name = match args.id {
        Some(id) => id.trim_start_matches('#').to_string(),
        None => {
            console::ask("譜面IDをプレフィックス込みで入力してください。");

            Input::<String>::with_theme(&ColorfulTheme::default())
                .allow_empty(false)
                .with_prompt("")
                .interact()
                .context("譜面IDの入力に失敗しました。")?
                .trim_start_matches('#')
                .to_string()
        }
    };
    let server = Server::guess(&name)?;

    console::info(&format!("{}{}{} から譜面を取得中...", rgb!(server.color), server.name, rgb!()));
    let level = server.fetch_level(&name).await?;
    console::info(&format!(
        "{} / {} - {} (Lv. {}) が選択されました。",
        level.info.title, level.info.artists, level.info.author, level.info.rating
    ));

    console::info("BGMを読み込んでいます...");
    let mut bgm_buf: Vec<u8> = Vec::new();
    if let Some(bgm_path) = args.bgm_override {
        let mut file = File::open(&bgm_path)
            .await
            .with_context(|| format!("BGMファイルを開けませんでした。（{}）", bgm_path))?;
        file.read_to_end(&mut bgm_buf)
            .await
            .with_context(|| format!("BGMファイルを読み込めませんでした。（{}）", bgm_path))?;
    } else {
        level.fetch_bgm(&mut bgm_buf).await?;
    }
    let bgm = Sound::load(&bgm_buf)? * args.bgm_volume;

    console::info("譜面を読み込んでいます...");
    let timing = pjsekai_soundgen_core::get_sound_timings(&level, args.shift).await?;

    console::info("効果音を読み込んでいます...");
    let effect = server.fetch_effect(level.info.engine.effect).await?;

    let progresses = MultiProgress::new();
    let mut progresses_map: HashMap<String, ProgressBar> = HashMap::new();
    let style = ProgressStyle::default_bar().progress_chars("- ");
    let rx = pjsekai_soundgen_core::synthesis(&timing, &effect, args.notes_per_thread).await;
    let threads = match rx.recv().context("合成の開始に失敗しました。")? {
        Progress::Info { threads } => threads,
        Progress::Failed { message } => bail!(message),
        _ => bail!("合成の開始に失敗しました。予期しない応答を受け取りました。"),
    };
    console::info(format!("{}スレッドで合成を開始します。", threads.len()).as_str());
    for (name, info) in threads.iter() {
        let progress =
            ProgressBar::new(info.max as u64)
                .with_style(style.clone().template(
                    LOG_STYLE.replace("{color_fg}", info.color.fg).replace("{color_bg}", info.color.bg).as_str(),
                ))
                .with_message(name.clone());
        progresses.add(progress.clone());
        progresses_map.insert(name.clone(), progress);
    }
    let draw_thread = thread::spawn(move || progresses.join());
    let mut merged_sounds = Sound::empty(None);
    while !progresses_map.is_empty() {
        match rx.recv().context("合成スレッドとの通信が切断されました。")? {
            Progress::Update { id, current } => {
                if let Some(progress) = progresses_map.get(&id) {
                    progress.set_position(current as u64);
                }
            }
            Progress::Finish { id, sound } => {
                if let Some(progress) = progresses_map.get(&id) {
                    progress.finish();
                }
                merged_sounds = merged_sounds.overlay_at(&sound, 0.0);
                progresses_map.remove(&id);
            }
            Progress::Failed { message } => bail!(message),
            Progress::Info { .. } => bail!("合成中に予期しない応答を受け取りました。"),
        }
    }
    // 進捗表示の失敗は生成結果に影響しないため、警告のみで続行する。
    match draw_thread.join() {
        Ok(Ok(())) => {}
        Ok(Err(err)) => console::warning(&format!("進捗表示の描画に失敗しました。: {}", err)),
        Err(_) => console::warning("進捗表示スレッドが異常終了しました。"),
    }
    console::info("合成が完了しました。");
    let mut final_bgm = if args.silent { Sound::empty(None) } else { bgm };
    final_bgm = final_bgm.overlay_at(&merged_sounds, 0.0);
    let output = args.output.unwrap_or(format!("dist/{}.mp3", name));
    console::info("出力しています...");
    final_bgm.export(output.as_str())?;
    console::info(format!("完了しました：{}", output).as_str());
    Ok(())
}
