use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
pub struct Srl {
    // sekai-best等、実ファイルを直接指すSrlにはhashが無いことがあるためOption化
    pub hash: Option<String>,
    pub url: String,
}
#[derive(Serialize, Deserialize)]
pub struct LevelListResponse {
    pub items: Vec<LevelInfo>,
    #[serde(rename = "pageCount")]
    pub page_count: i32,
}
#[derive(Serialize, Deserialize)]
pub struct ItemResponse<T> {
    pub item: T,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LevelEntity {
    pub archetype: String,
    pub data: Vec<LevelEntityData>,
    pub name: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LevelEntityData {
    pub name: String,
    pub value: Option<f32>,
    pub r#ref: Option<String>,
}
#[derive(Serialize, Deserialize)]
pub struct EffectData {
    pub clips: Vec<EffectClip>,
}
#[derive(Serialize, Deserialize)]
pub struct EffectClip {
    pub name: String,
    pub filename: String,
}
impl LevelEntity {
    pub fn get_value(&self, key: &str) -> Option<f32> {
        for data in self.data.iter() {
            if data.name == key {
                data.value?;
                return Some(data.value.unwrap());
            }
        }
        None
    }
    pub fn get_ref_raw(&self, key: &str) -> Option<String> {
        for data in self.data.iter() {
            if data.name == key {
                let r#ref = data.r#ref.as_ref()?;
                return Some(r#ref.to_string());
            }
        }
        None
    }
    pub fn get_ref(&self, entities: &[LevelEntity], key: &str) -> Option<LevelEntity> {
        let ref_raw = self.get_ref_raw(key)?;
        for entity in entities.iter() {
            if entity.name.as_ref().is_some_and(|name| name == &ref_raw) {
                return Some(entity.clone());
            }
        }
        None
    }
}
#[derive(Debug, Serialize, Deserialize)]
pub struct LevelData {
    #[serde(rename = "bgmOffset")]
    pub bgm_offset: f32,
    pub entities: Vec<LevelEntity>,
}
#[derive(Serialize, Deserialize)]
pub struct LevelInfo {
    pub title: String,
    pub artists: String,
    pub author: String,
    pub name: String,
    pub rating: i32,
    pub bgm: Srl,
    pub data: Srl,
    pub engine: EngineInfo,
}
#[derive(Serialize, Deserialize)]
pub struct EngineInfo {
    pub version: i32,
    pub effect: EffectInfo,
}
#[derive(Serialize, Deserialize)]
pub struct EffectInfo {
    pub audio: Srl,
    pub data: Srl,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entity(json: serde_json::Value) -> LevelEntity {
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn get_value_returns_value_for_matching_key() {
        let e = entity(serde_json::json!({
            "archetype": "NormalTapNote",
            "data": [{"name": "#BEAT", "value": 1.5}],
            "name": null,
        }));
        assert_eq!(e.get_value("#BEAT"), Some(1.5));
    }

    #[test]
    fn get_value_returns_none_for_missing_key() {
        let e = entity(serde_json::json!({
            "archetype": "NormalTapNote",
            "data": [{"name": "#BEAT", "value": 1.5}],
            "name": null,
        }));
        assert_eq!(e.get_value("#BPM"), None);
    }

    #[test]
    fn get_value_returns_none_when_value_is_absent() {
        let e = entity(serde_json::json!({
            "archetype": "NormalTapNote",
            "data": [{"name": "#BEAT", "ref": "other"}],
            "name": null,
        }));
        assert_eq!(e.get_value("#BEAT"), None);
    }

    #[test]
    fn get_ref_raw_returns_ref_for_matching_key() {
        let e = entity(serde_json::json!({
            "archetype": "NormalSlideConnector",
            "data": [{"name": "head", "ref": "a"}],
            "name": null,
        }));
        assert_eq!(e.get_ref_raw("head"), Some("a".to_string()));
        assert_eq!(e.get_ref_raw("tail"), None);
    }

    #[test]
    fn get_ref_raw_returns_none_when_ref_is_absent() {
        let e = entity(serde_json::json!({
            "archetype": "NormalSlideConnector",
            "data": [{"name": "head", "value": 1.0}],
            "name": null,
        }));
        assert_eq!(e.get_ref_raw("head"), None);
    }

    #[test]
    fn get_ref_resolves_named_entity() {
        let connector = entity(serde_json::json!({
            "archetype": "NormalSlideConnector",
            "data": [{"name": "head", "ref": "a"}],
            "name": null,
        }));
        let head = entity(serde_json::json!({
            "archetype": "NormalSlideStartNote",
            "data": [{"name": "#BEAT", "value": 2.0}],
            "name": "a",
        }));
        let entities = vec![connector.clone(), head];
        let resolved = connector.get_ref(&entities, "head").unwrap();
        assert_eq!(resolved.archetype, "NormalSlideStartNote");
        assert_eq!(resolved.get_value("#BEAT"), Some(2.0));
    }

    #[test]
    fn get_ref_returns_none_when_target_is_missing() {
        let connector = entity(serde_json::json!({
            "archetype": "NormalSlideConnector",
            "data": [{"name": "head", "ref": "a"}],
            "name": null,
        }));
        let entities = vec![connector.clone()];
        assert!(connector.get_ref(&entities, "head").is_none());
    }

    #[test]
    fn srl_deserializes_without_hash() {
        let srl: Srl = serde_json::from_str(r#"{"url": "https://example.com/bgm.mp3"}"#).unwrap();
        assert_eq!(srl.hash, None);
        assert_eq!(srl.url, "https://example.com/bgm.mp3");
    }

    #[test]
    fn level_data_deserializes_bgm_offset() {
        let data: LevelData = serde_json::from_str(r#"{"bgmOffset": 0.25, "entities": []}"#).unwrap();
        assert_eq!(data.bgm_offset, 0.25);
        assert!(data.entities.is_empty());
    }
}
