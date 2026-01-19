use std::sync::LazyLock;

use reqwest::Client;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AktoolResponse {
    pub code: i32,

    #[serde(default)]
    pub data: Option<Vec<TradeRecord>>,
}

#[derive(Debug, Deserialize)]
pub struct TradeRecord {
    pub signature: String,

    #[serde(rename = "signatureUser")]
    pub signature_user: String,

    pub mint: String,

    pub slot: u64,
    pub index: u32,

    /// 字符串数字，防止精度问题
    pub amount: String,

    #[serde(rename = "isBuy")]
    pub is_buy: i32,

    #[serde(rename = "isSuccess")]
    pub is_success: bool,

    pub price: String,

    #[serde(rename = "blockTime")]
    pub block_time: i64,

    /// 可选字段（有的失败交易可能没有）
    #[serde(default)]
    pub tip: Option<String>,

    #[serde(default)]
    pub gasFee: Option<String>,
}

pub static AKBOT_KEY: LazyLock<String> = LazyLock::new(|| {
    std::env::var("AKBOT_KEY")
        .expect("AKBOT_KEY environment variable not set")
});

pub async fn aktool_search(param: &str) -> Result<AktoolResponse, reqwest::Error> {
    let client = reqwest::Client::new();

    let resp = client
        .get("https://api.aktool.pro/search")
        .query(&[("key", AKBOT_KEY.as_ref()), ("param", param)])
        .header("Accept", "*/*")
        .header("Origin", "https://node1.aktool.pro")
        .header("Referer", "https://node1.aktool.pro/")
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
                 AppleWebKit/537.36 (KHTML, like Gecko) \
                 Chrome/143.0.0.0 Safari/537.36",
        )
        .send()
        .await?
        .error_for_status()?
        .json::<AktoolResponse>()
        .await?;

    Ok(resp)
}