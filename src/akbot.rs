use std::sync::LazyLock;

use reqwest::Client;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AktoolResponse {
    pub code: i32,

    #[serde(default)]
    pub data: Option<Vec<TradeRecordRaw>>,
}

#[derive(Debug, Deserialize)]
pub struct TradeRecordRaw {
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

#[derive(Debug, Deserialize)]
pub struct TradeRecord {
    pub signature: String,

    #[serde(rename = "signatureUser")]
    pub signature_user: String,

    pub mint: String,

    pub slot: u64,
    pub index: u32,

    /// 字符串数字，防止精度问题
    pub amount: f64,

    #[serde(rename = "isBuy")]
    pub is_buy: i32,

    #[serde(rename = "isSuccess")]
    pub is_success: bool,

    pub price: f64,

    #[serde(rename = "blockTime")]
    pub block_time: i64,

    /// 可选字段（有的失败交易可能没有）
    #[serde(default)]
    pub tip: Option<f64>,

    #[serde(default)]
    pub gasFee: Option<f64>,
}

impl From<TradeRecordRaw> for TradeRecord {
    fn from(raw: TradeRecordRaw) -> Self {
        TradeRecord {
            signature: raw.signature,
            signature_user: raw.signature_user,
            mint: raw.mint,
            slot: raw.slot,
            index: raw.index,
            amount: raw.amount.parse().unwrap_or(0.0), // 失败默认 0
            is_buy: raw.is_buy,
            is_success: raw.is_success,
            price: raw.price.parse().unwrap_or(0.0),
            block_time: raw.block_time,
            tip: raw.tip.and_then(|s| s.parse().ok()),
            gasFee: raw.gasFee.and_then(|s| s.parse().ok()),
        }
    }
}

pub static AKBOT_KEY: LazyLock<String> =
    LazyLock::new(|| std::env::var("AKBOT_KEY").expect("AKBOT_KEY environment variable not set"));

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
