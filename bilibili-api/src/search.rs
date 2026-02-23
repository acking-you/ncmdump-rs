//! Bilibili video search.

use crate::client::BilibiliClient;
use crate::error::Result;
use crate::types::{SearchResult, VideoItem};

impl BilibiliClient {
    /// Search for videos by keyword.
    ///
    /// Uses `/x/web-interface/wbi/search/type` with `search_type=video`.
    pub fn search_video(&self, keyword: &str, page: u64, page_size: u64) -> Result<SearchResult> {
        let params = vec![
            ("search_type".into(), "video".into()),
            ("keyword".into(), keyword.to_owned()),
            ("page".into(), page.to_string()),
            ("page_size".into(), page_size.to_string()),
        ];

        let resp = self.wbi_get("/x/web-interface/wbi/search/type", &params)?;
        let data = &resp["data"];

        let num_results = data["numResults"]
            .as_u64()
            .or_else(|| {
                data["numResults"]
                    .as_i64()
                    .map(|n| u64::try_from(n).unwrap_or(0))
            })
            .unwrap_or(0);

        let results: Vec<VideoItem> = data["result"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| serde_json::from_value(v.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();

        Ok(SearchResult {
            num_results,
            page: data["page"].as_u64().unwrap_or(page),
            page_size: data["pagesize"].as_u64().unwrap_or(page_size),
            results,
        })
    }
}
