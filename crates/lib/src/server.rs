use crate::level::Level;
use crate::sonolus::{EffectData, EffectInfo, ItemResponse, LevelData, LevelInfo, Srl};
use crate::sound::Effect;
use crate::utils::{debug, fetch_bytes, gunzip_json};

use anyhow::Result;
use dirs::cache_dir;
use once_cell::sync::Lazy;
use std::path::Path;
use tokio::try_join;

#[derive(Debug, Clone)]
pub struct Server {
    pub id: String,
    pub name: String,
    pub color: i32,
    pub url: String,
}

static CACHE_DIR: Lazy<Box<Path>> = Lazy::new(|| {
    let mut path = cache_dir().or_else(|| "./cache".parse().ok()).unwrap();
    path.push("pjsekai-soundgen-rust");
    path.into_boxed_path()
});

struct KnownServer {
    prefix: &'static str,
    id: &'static str,
    name: &'static str,
    color: i32,
    url: &'static str,
}

static KNOWN_SERVERS: &[KnownServer] = &[
    KnownServer {
        prefix: "frpt-",
        id: "potato_leaves",
        name: "Potato Leaves",
        color: 0x88cb7f,
        url: "https://ptlv.milkbun.org",
    },
    KnownServer {
        prefix: "chcy-",
        id: "chart_cyanvas",
        name: "Chart Cyanvas",
        color: 0x83ccd2,
        url: "https://cc.milkbun.org/",
    },
    KnownServer {
        prefix: "UnCh-",
        id: "untitledCharts",
        name: "UntitledCharts",
        color: 0x7765da,
        url: "https://untitledcharts.com",
    },
    KnownServer {
        prefix: "coconut-next-sekai-",
        id: "next_sekai",
        name: "Next SEKAI",
        color: 0x02cbbd,
        url: "https://coconut.sonolus.com/next-sekai",
    },
    KnownServer {
        prefix: "sss-",
        id: "sbuga_sonolus",
        name: "Sbuga's Sonolus Server",
        color: 0xe0f2fe,
        url: "https://sonolus.sbuga.com",
    },
    KnownServer {
        prefix: "local-",
        id: "ScoreSync",
        name: "ScoreSync",
        color: 0x545454,
        url: "http://localhost:3939",
    },
];

impl Server {
    pub fn guess(level_name: &str) -> Result<Server> {
        KNOWN_SERVERS
            .iter()
            .find(|server| level_name.starts_with(server.prefix))
            .map(|server| Server {
                id: server.id.to_string(),
                name: server.name.to_string(),
                color: server.color,
                url: server.url.to_string(),
            })
            .ok_or_else(|| anyhow::anyhow!("サーバーを特定できませんでした。"))
    }

    async fn fetch_srl_with_cache(&self, srl: &Srl) -> Result<Vec<u8>> {
        // hashが無い(sekai-best等、実ファイルを直接指すSrl)場合はurlをキーとして使う
        let key_source = srl.hash.clone().unwrap_or_else(|| srl.url.clone());
        let key = format!("{}-{}", self.id, key_source);

        debug!(&key);

        // ScoreSyncの場合はキャッシュを使わず常に取得
        if self.id != "ScoreSync" {
            let cache_path = CACHE_DIR.join(&key);
            if let Ok(cache) = tokio::fs::read(&cache_path).await {
                debug!("cache hit");
                return Ok(cache);
            }
            debug!("cache miss");
        } else {
            debug!("ScoreSync: always fetch from server (no cache)");
        }

        let url = self.merge_url(&srl.url);
        debug!(&url);
        let bytes = fetch_bytes(&url, "データの取得に失敗しました。").await?;

        if self.id != "ScoreSync" {
            tokio::fs::create_dir_all(CACHE_DIR.as_ref()).await?;
            tokio::fs::write(CACHE_DIR.join(&key), &bytes).await?;
        }

        Ok(bytes)
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
        let level_info = client
            .get(format!("{}/sonolus/levels/{}", self.url, api_level_name).as_str())
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("譜面情報の取得に失敗しました。: {}", e))?
            .json::<ItemResponse<LevelInfo>>()
            .await
            .map_err(|e| anyhow::anyhow!("譜面情報の取得に失敗しました。: {}", e))?
            .item;
        let data_bytes = &self
            .fetch_srl_with_cache(&level_info.data)
            .await
            .map_err(|e| anyhow::anyhow!("譜面データの取得に失敗しました。: {}", e))?;

        let level_data = gunzip_json::<LevelData>(data_bytes, "譜面データの取得に失敗しました。")?;

        Ok(Level::new(self.clone(), level_info, level_data))
    }

    pub fn merge_url(&self, path: &str) -> String {
        if path.starts_with("http") {
            path.to_string()
        } else {
            format!("{}{}", self.url.trim_end_matches('/'), path)
        }
    }

    pub async fn fetch_effect(&self, effect: EffectInfo) -> Result<Effect> {
        let (data_compressed, audio) =
            try_join!(self.fetch_srl_with_cache(&effect.data), self.fetch_srl_with_cache(&effect.audio))
                .map_err(|e| anyhow::anyhow!("効果音の取得に失敗しました。: {}", e))?;

        let zip = zip::ZipArchive::new(std::io::Cursor::new(audio))
            .map_err(|e| anyhow::anyhow!("効果音の取得に失敗しました。: {}", e))?;

        let data = gunzip_json::<EffectData>(&data_compressed, "効果音の取得に失敗しました。")?;

        Effect::new(data, zip)
    }
}
