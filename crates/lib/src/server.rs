use crate::level::Level;
use crate::sonolus::{EffectData, EffectInfo, ItemResponse, LevelData, LevelInfo, Srl};
use crate::sound::Effect;
use crate::utils::debug;

use anyhow::Result;
use dirs::cache_dir;
use flate2::read::GzDecoder;
use once_cell::sync::Lazy;
use std::io::Read;
use std::path::{Path, PathBuf};
use tokio::try_join;

#[derive(Debug, Clone)]
pub struct Server {
    pub id: String,
    pub name: String,
    pub color: i32,
    pub url: String,
}

static CACHE_DIR: Lazy<Box<Path>> = Lazy::new(|| {
    let mut path = cache_dir().unwrap_or_else(|| PathBuf::from("./cache"));
    path.push("pjsekai-soundgen-rust");
    path.into_boxed_path()
});

impl Server {
    pub fn guess(level_name: &str) -> Result<Server> {
        if level_name.starts_with("frpt-") {
            Ok(Server {
                id: "potato_leaves".to_string(),
                name: "Potato Leaves".to_string(),
                color: 0x88cb7f,
                url: "https://ptlv.milkbun.org".to_string(),
            })
        } else if level_name.starts_with("chcy-") {
            Ok(Server {
                id: "chart_cyanvas".to_string(),
                name: "Chart Cyanvas".to_string(),
                color: 0x83ccd2,
                url: "https://cc.milkbun.org/".to_string(),
            })
        } else if level_name.starts_with("UnCh-") {
            Ok(Server {
                id: "untitledCharts".to_string(),
                name: "UntitledCharts".to_string(),
                color: 0x7765da,
                url: "https://untitledcharts.com".to_string(),
            })
        } else if level_name.starts_with("coconut-next-sekai-") {
            Ok(Server {
                id: "next_sekai".to_string(),
                name: "Next SEKAI".to_string(),
                color: 0x02cbbd,
                url: "https://coconut.sonolus.com/next-sekai".to_string(),
            })
        } else if level_name.starts_with("sss-") {
            Ok(Server {
                id: "sbuga_sonolus".to_string(),
                name: "Sbuga's Sonolus Server".to_string(),
                color: 0xe0f2fe,
                url: "https://sonolus.sbuga.com".to_string(),
            })
        } else if level_name.starts_with("local-") {
            Ok(Server {
                id: "ScoreSync".to_string(),
                name: "ScoreSync".to_string(),
                color: 0x545454,
                url: "http://localhost:3939".to_string(),
            })
        } else {
            Err(anyhow::anyhow!("サーバーを特定できませんでした。"))
        }
    }

    async fn fetch_srl_with_cache(&self, srl: &Srl) -> Result<Vec<u8>> {
        // hashが無い(sekai-best等、実ファイルを直接指すSrl)場合はurlをキーとして使う
        let key_source = srl.hash.clone().unwrap_or_else(|| srl.url.clone());
        let key = format!("{}-{}", self.id, key_source);

        debug!(&key);

        // ScoreSyncの場合はキャッシュを使わず常に取得
        if self.id != "ScoreSync" {
            let cache_path = CACHE_DIR.join(&key);
            match tokio::fs::read(&cache_path).await {
                Ok(cache) => {
                    debug!("cache hit");
                    return Ok(cache);
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    debug!("cache miss");
                }
                // キャッシュは失敗しても取得し直せばよいが、NotFound以外は黙って無視せず知らせる。
                Err(e) => {
                    eprintln!(
                        "警告：キャッシュの読み込みに失敗しました。サーバーから取得します。({}): {}",
                        cache_path.display(),
                        e
                    );
                }
            }
        } else {
            debug!("ScoreSync: always fetch from server (no cache)");
        }

        let client = reqwest::Client::new();
        let url = self.merge_url(&srl.url);
        debug!(&url);
        let bgm_response =
            client.get(url).send().await.map_err(|e| anyhow::anyhow!("データの取得に失敗しました。: {}", e))?;

        if !bgm_response.status().is_success() {
            return Err(anyhow::anyhow!("データの取得に失敗しました。（HTTP {}）", bgm_response.status()));
        }

        let bytes = bgm_response
            .bytes()
            .await
            .map_err(|e| anyhow::anyhow!("データの取得に失敗しました。: {}", e))?
            .to_vec();

        // キャッシュの保存に失敗しても取得したデータ自体は使えるので、警告だけして続行する。
        if self.id != "ScoreSync" {
            if let Err(e) = self.write_cache(&key, &bytes).await {
                eprintln!("警告：キャッシュの保存に失敗しました。: {}", e);
            }
        }

        Ok(bytes)
    }

    async fn write_cache(&self, key: &str, bytes: &[u8]) -> Result<()> {
        tokio::fs::create_dir_all(CACHE_DIR.as_ref()).await?;
        tokio::fs::write(CACHE_DIR.join(key), bytes).await?;
        Ok(())
    }

    pub async fn fetch_level(&self, level_name: &str) -> Result<Level> {
        let client = reqwest::Client::new();

        // ScoreSyncの場合は、prefixを除去
        let api_level_name = if self.id == "ScoreSync" && level_name.starts_with("local-") {
            &level_name["local-".len()..]
        } else {
            level_name
        };

        // 譜面情報を取得
        let level_info_response = client
            .get(format!("{}/sonolus/levels/{}", self.url, api_level_name).as_str())
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("譜面情報の取得に失敗しました。: {}", e))?;

        if !level_info_response.status().is_success() {
            return Err(anyhow::anyhow!(
                "譜面情報の取得に失敗しました。譜面IDを確認してください。（HTTP {}）",
                level_info_response.status()
            ));
        }

        let level_info = level_info_response
            .json::<ItemResponse<LevelInfo>>()
            .await
            .map_err(|e| anyhow::anyhow!("譜面情報の取得に失敗しました。: {}", e))?
            .item;
        let data_bytes = &self
            .fetch_srl_with_cache(&level_info.data)
            .await
            .map_err(|e| anyhow::anyhow!("譜面データの取得に失敗しました。: {}", e))?;

        let mut data_raw = GzDecoder::new(&data_bytes[..]);
        let mut buf = Vec::new();
        data_raw
            .read_to_end(&mut buf)
            .map_err(|e| anyhow::anyhow!("譜面データの取得に失敗しました。: {}", e))?;

        let level_data = serde_json::from_slice::<LevelData>(&buf[..])
            .map_err(|e| anyhow::anyhow!("譜面データの取得に失敗しました。: {}", e))?;

        Ok(Level::new(self.clone(), level_info, level_data))
    }

    pub fn merge_url(&self, path: &str) -> String {
        if path.starts_with("http") {
            path.to_string()
        } else if path.starts_with("/") {
            let url = self.url.trim_end_matches('/');
            format!("{}{}", url, path)
        } else {
            let url = self.url.trim_end_matches('/');
            format!("{}{}", url, path)
        }
    }

    pub async fn fetch_effect(&self, effect: EffectInfo) -> Result<Effect> {
        let (data_compressed, audio) =
            try_join!(self.fetch_srl_with_cache(&effect.data), self.fetch_srl_with_cache(&effect.audio))
                .map_err(|e| anyhow::anyhow!("効果音の取得に失敗しました。: {}", e))?;

        let zip = zip::ZipArchive::new(std::io::Cursor::new(audio))
            .map_err(|e| anyhow::anyhow!("効果音の取得に失敗しました。: {}", e))?;

        let mut data_raw = GzDecoder::new(&data_compressed[..]);
        let mut buf = Vec::new();
        data_raw.read_to_end(&mut buf).map_err(|e| anyhow::anyhow!("効果音の取得に失敗しました。: {}", e))?;
        let data = serde_json::from_slice::<EffectData>(&buf[..])
            .map_err(|e| anyhow::anyhow!("効果音の取得に失敗しました。: {}", e))?;

        Effect::new(data, zip)
    }
}
