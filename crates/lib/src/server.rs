use crate::level::Level;
use crate::sonolus::{EffectData, EffectInfo, ItemResponse, LevelData, LevelInfo, Srl};
use crate::sound::Effect;
use crate::utils::debug;

use anyhow::Result;
use dirs::cache_dir;
use flate2::read::GzDecoder;
use once_cell::sync::Lazy;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::Read;
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

/// ダウンロードされるデータのサイズ上限（圧縮時・展開後の両方に適用）。
const MAX_DOWNLOAD_SIZE: usize = 512 * 1024 * 1024;
const MAX_DECOMPRESSED_SIZE: u64 = 512 * 1024 * 1024;

/// サーバーから与えられた文字列をファイル名として安全な形に変換する。
/// 英数字・`-`・`_`以外は`_`に置き換え、パス区切りや`..`が混入しないようにする。
fn sanitize_cache_key(key: &str) -> String {
    let sanitized: String = key
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    // 長すぎるキーはファイルシステムの制限に触れるため切り詰める。
    sanitized.chars().take(128).collect()
}

/// 譜面IDをURLのパスセグメントとして安全に埋め込んだURLを作る。
/// セグメントとしてエンコードすることで、`../`によるパスの抜け出しやクエリ・フラグメントの注入を防ぐ。
fn level_api_url(base: &str, level_name: &str) -> Result<reqwest::Url> {
    if level_name.is_empty() {
        return Err(anyhow::anyhow!("譜面IDが空です。"));
    }
    if level_name.chars().any(|c| c.is_control()) {
        return Err(anyhow::anyhow!("譜面IDに使用できない文字が含まれています。"));
    }
    let mut url = reqwest::Url::parse(base).map_err(|e| anyhow::anyhow!("サーバーURLが不正です。: {}", e))?;
    url.path_segments_mut()
        .map_err(|_| anyhow::anyhow!("サーバーURLが不正です。"))?
        .pop_if_empty()
        .extend(["sonolus", "levels", level_name]);
    Ok(url)
}

/// レスポンスボディを上限付きで読み込む。
pub(crate) async fn read_body_limited(response: reqwest::Response, limit: usize) -> Result<Vec<u8>> {
    let mut response = response;
    let mut buf: Vec<u8> = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if buf.len() + chunk.len() > limit {
            return Err(anyhow::anyhow!("データが大きすぎます。"));
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(buf)
}

/// gzipデータを上限付きで展開する。展開爆弾によるメモリ枯渇を防ぐ。
fn decompress_limited(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    let mut reader = GzDecoder::new(bytes).take(MAX_DECOMPRESSED_SIZE + 1);
    reader.read_to_end(&mut buf)?;
    if buf.len() as u64 > MAX_DECOMPRESSED_SIZE {
        return Err(anyhow::anyhow!("データが大きすぎます。"));
    }
    Ok(buf)
}

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
        // キーはサーバーから与えられる値なので、キャッシュディレクトリ外を指せないようにする。
        // サニタイズで失われた情報による衝突を避けるため、元の値のハッシュを付加する。
        let mut hasher = DefaultHasher::new();
        key_source.hash(&mut hasher);
        let key =
            format!("{}-{}-{:016x}", sanitize_cache_key(&self.id), sanitize_cache_key(&key_source), hasher.finish());

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

        let client = reqwest::Client::new();
        let url = self.merge_url(&srl.url);
        debug!(&url);
        let bgm_response =
            client.get(url).send().await.map_err(|e| anyhow::anyhow!("データの取得に失敗しました。: {}", e))?;

        if !bgm_response.status().is_success() {
            return Err(anyhow::anyhow!("データの取得に失敗しました。"));
        }

        let bytes = read_body_limited(bgm_response, MAX_DOWNLOAD_SIZE)
            .await
            .map_err(|e| anyhow::anyhow!("データの取得に失敗しました。: {}", e))?;

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
        let level_url = level_api_url(&self.url, api_level_name)?;

        // 譜面情報を取得
        let level_info = client
            .get(level_url)
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

        let buf = decompress_limited(&data_bytes[..])
            .map_err(|e| anyhow::anyhow!("譜面データの取得に失敗しました。: {}", e))?;

        let level_data = serde_json::from_slice::<LevelData>(&buf[..])
            .map_err(|e| anyhow::anyhow!("譜面データの取得に失敗しました。: {}", e))?;

        Ok(Level::new(self.clone(), level_info, level_data))
    }

    pub fn merge_url(&self, path: &str) -> String {
        if path.starts_with("http://") || path.starts_with("https://") {
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

        let buf = decompress_limited(&data_compressed[..])
            .map_err(|e| anyhow::anyhow!("効果音の取得に失敗しました。: {}", e))?;
        let data = serde_json::from_slice::<EffectData>(&buf[..])
            .map_err(|e| anyhow::anyhow!("効果音の取得に失敗しました。: {}", e))?;

        Effect::new(data, zip)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_api_url_encodes_path_traversal() {
        let url = level_api_url("https://example.com", "../../admin").unwrap();
        assert_eq!(url.as_str(), "https://example.com/sonolus/levels/..%2F..%2Fadmin");
    }

    #[test]
    fn level_api_url_encodes_query_injection() {
        let url = level_api_url("https://example.com/", "abc?x=1#y").unwrap();
        assert_eq!(url.as_str(), "https://example.com/sonolus/levels/abc%3Fx=1%23y");
    }

    #[test]
    fn level_api_url_keeps_sub_path_of_server() {
        let url = level_api_url("https://coconut.sonolus.com/next-sekai", "abc").unwrap();
        assert_eq!(url.as_str(), "https://coconut.sonolus.com/next-sekai/sonolus/levels/abc");
    }

    #[test]
    fn sanitize_cache_key_removes_path_separators() {
        assert_eq!(sanitize_cache_key("../../etc/passwd"), "______etc_passwd");
        assert_eq!(sanitize_cache_key("/absolute"), "_absolute");
        assert_eq!(sanitize_cache_key("abc123-_"), "abc123-_");
    }

    #[test]
    fn cache_path_stays_inside_cache_dir() {
        let srl = Srl {
            hash: Some("../../../../tmp/pwned".to_string()),
            url: "/data".to_string(),
        };
        let key_source = srl.hash.clone().unwrap();
        let key = sanitize_cache_key(&key_source);
        assert!(!CACHE_DIR.join(key).starts_with("/tmp"));
    }
}
