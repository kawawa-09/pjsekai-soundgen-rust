use crate::{
    server::Server,
    sonolus::{LevelData, LevelInfo},
};
use anyhow::Result;

pub struct Level {
    pub server: Server,
    pub info: LevelInfo,
    pub data: LevelData,
}

impl Level {
    pub fn new(server: Server, info: LevelInfo, data: LevelData) -> Self {
        Self { server, info, data }
    }

    pub async fn fetch_bgm(&self, buf: &mut Vec<u8>) -> Result<()> {
        let url = self.server.merge_url(&self.info.bgm.url);
        buf.append(&mut crate::utils::fetch_bytes(&url, "BGMの取得に失敗しました。").await?);
        Ok(())
    }
}
