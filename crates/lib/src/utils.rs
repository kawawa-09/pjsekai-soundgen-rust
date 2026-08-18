use anyhow::Result;
use flate2::read::GzDecoder;
use serde::de::DeserializeOwned;
use std::io::Read;

#[cfg(debug_assertions)]
macro_rules! debug {
    ($($arg:tt)*) => {
        dbg!($($arg)*)
    };
}

#[cfg(not(debug_assertions))]
macro_rules! debug {
    ($($arg:tt)*) => {};
}

pub(crate) use debug;

/// URLからバイト列を取得する。失敗時は error_message をプレフィックスにしたエラーを返す。
pub(crate) async fn fetch_bytes(url: &str, error_message: &str) -> Result<Vec<u8>> {
    let client = reqwest::Client::new();
    let response = client.get(url).send().await.map_err(|e| anyhow::anyhow!("{}: {}", error_message, e))?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!("{}", error_message));
    }

    Ok(response.bytes().await.map_err(|e| anyhow::anyhow!("{}: {}", error_message, e))?.to_vec())
}

/// gzip圧縮されたJSONバイト列を展開してデシリアライズする。
pub(crate) fn gunzip_json<T: DeserializeOwned>(bytes: &[u8], error_message: &str) -> Result<T> {
    let mut buf = Vec::new();
    GzDecoder::new(bytes)
        .read_to_end(&mut buf)
        .map_err(|e| anyhow::anyhow!("{}: {}", error_message, e))?;
    serde_json::from_slice::<T>(&buf).map_err(|e| anyhow::anyhow!("{}: {}", error_message, e))
}
