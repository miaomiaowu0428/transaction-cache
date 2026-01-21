use log::info;
use serde::{Deserialize, Serialize};
use solana_client::rpc_client::GetConfirmedSignaturesForAddress2Config;
use solana_client::rpc_config::{CommitmentConfig, RpcTransactionConfig, UiTransactionEncoding};
use solana_client::rpc_response::RpcConfirmedTransactionStatusWithSignature;
use solana_sdk::signature::Signature;
use solana_transaction_status_client_types::{
    EncodedConfirmedTransactionWithStatusMeta, EncodedTransactionWithStatusMeta,
    TransactionDetails, UiConfirmedBlock,
};
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;
use std::{collections::HashMap, sync::LazyLock};
use tokio::fs;
use tokio::sync::RwLock;
pub use utils::JSON_RPC_CLIENT;
use utils::log_time;
pub mod akbot;
pub mod tx_fetcher_v2;
pub mod type_wraps;

/// 通过RPC获取TxDetail（可作为get_tx_detail_or_fetch的fetch回调）
pub async fn fetch_tx_detail_from_rpc(sig: &Signature) -> anyhow::Result<Option<TxDetailLocal>> {
    let result = JSON_RPC_CLIENT
        .get_transaction_with_config(
            sig,
            RpcTransactionConfig {
                encoding: Some(UiTransactionEncoding::Json),
                max_supported_transaction_version: Some(0),
                commitment: None,
            },
        )
        .await;
    match result {
        Ok(tx) => Ok(Some(TxDetailLocal::from(tx))),
        Err(e) => {
            log::warn!("fetch {} error: {e}", sig);
            Ok(None)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TxDetailLocal {
    pub slot: u64,
    #[serde(flatten)]
    pub transaction: EncodedTransactionWithStatusMeta,
    pub block_time: Option<i64>,
}

impl From<EncodedConfirmedTransactionWithStatusMeta> for TxDetailLocal {
    fn from(e: EncodedConfirmedTransactionWithStatusMeta) -> Self {
        Self {
            slot: e.slot,
            transaction: e.transaction,
            block_time: e.block_time,
        }
    }
}

impl From<TxDetailLocal> for EncodedConfirmedTransactionWithStatusMeta {
    fn from(l: TxDetailLocal) -> Self {
        Self {
            slot: l.slot,
            transaction: l.transaction,
            block_time: l.block_time,
        }
    }
}

pub type TxDetail = TxDetailLocal;

// 缓存类型
pub static TX_DETAIL_CACHE: LazyLock<RwLock<HashMap<Signature, TxDetail>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static INITED: LazyLock<RwLock<bool>> = LazyLock::new(|| RwLock::new(false));
const LOCAL_CACHE_PATH: &str = "allFetchedTxs.json";

/// 初始化缓存：从本地JSON加载HashMap<Signature, TxDetail>
async fn init_tx_cache() -> anyhow::Result<()> {
    let path = Path::new(LOCAL_CACHE_PATH);
    // 新增：文件不存在则创建空的JSON文件（写入{}），并标记已初始化
    if !path.exists() {
        // 写入空的JSON对象，避免后续序列化/反序列化报错
        fs::write(path, "{}").await?;
        // 标记为已初始化，防止后续重复调用init_tx_cache
        let mut write = INITED.write().await;
        *write = true;
    }
    let mut write = INITED.write().await;
    if *write {
        return Ok(());
    } else {
        *write = true;
    }
    let content = fs::read_to_string(LOCAL_CACHE_PATH).await?;
    let raw_map: HashMap<String, TxDetail> = serde_json::from_str(&content)?;
    let mut cache = TX_DETAIL_CACHE.write().await;
    for (k, v) in raw_map {
        if let Ok(sig) = k.parse::<Signature>() {
            cache.insert(sig, v);
        }
    }
    Ok(())
}

/// 查询TxDetail，优先本地缓存，不存在则自动fetch（带重试）并写入缓存
pub async fn get_tx(sig: &Signature) -> anyhow::Result<Option<TxDetail>> {
    use solana_client::rpc_config::{RpcTransactionConfig, UiTransactionEncoding};
    use std::time::Duration;
    use tokio::time::sleep;

    if *INITED.read().await == false {
        init_tx_cache().await.unwrap()
    }

    let cache = TX_DETAIL_CACHE.read().await;
    if let Some(detail) = cache.get(sig) {
        info!("found in cache: {sig}; cache size: {}", cache.len());
        return Ok(Some(detail.clone()));
    }
    drop(cache);
    // fetch with retry
    log_time!("fetching time cost: ", {
        info!("feching: {sig}");
        let mut retry_times = 3;
        let mut last_err = None;
        let mut fetched: Option<TxDetail> = None;
        while retry_times > 0 {
            let result = JSON_RPC_CLIENT
                .get_transaction_with_config(
                    sig,
                    RpcTransactionConfig {
                        encoding: Some(UiTransactionEncoding::Json),
                        max_supported_transaction_version: Some(0),
                        commitment: None,
                    },
                )
                .await;
            match result {
                Ok(tx) => {
                    fetched = Some(TxDetail::from(tx));
                    break;
                }
                Err(e) => {
                    retry_times -= 1;
                    last_err = Some(e);
                    if retry_times == 0 {
                        break;
                    }
                    sleep(Duration::from_millis(100)).await;
                }
            }
        }
        if let Some(detail) = fetched {
            TX_DETAIL_CACHE
                .write()
                .await
                .insert(sig.clone(), detail.clone());
            save_cache().await?;
            Ok(Some(detail))
        } else {
            if let Some(e) = last_err {
                log::warn!("fetch {} error: {e}", sig);
            }
            Ok(None)
        }
    })
}

/// 持久化缓存到本地
pub async fn save_cache() -> anyhow::Result<()> {
    let cache = TX_DETAIL_CACHE.read().await;
    let map: HashMap<String, &TxDetail> = cache.iter().map(|(k, v)| (k.to_string(), v)).collect();
    let content = serde_json::to_string_pretty(&map)?;
    fs::write(LOCAL_CACHE_PATH, content).await?;
    Ok(())
}

use anyhow::Result;
use solana_sdk::pubkey::Pubkey;

// 传入 RPC 客户端、用户地址、可选的最大获取条数（None 表示获取全部）
pub async fn tx_for_address(address: &Pubkey, max_count: Option<usize>) -> Result<Vec<Signature>> {
    // 存储最终的交易签名列表
    let mut signatures = Vec::new();
    // 分页查询的 before 参数（初始为 None，后续为上一批次最旧的签名）
    let mut before_signature: Option<Signature> = None;

    loop {
        // 构建 RPC 查询配置，每次最多查 1000 条（Solana RPC 最大限制）
        let config = GetConfirmedSignaturesForAddress2Config {
            limit: Some(1000),
            before: before_signature.clone(),
            until: None,
            commitment: None,
        };

        // 调用 RPC 获取签名批次，失败时重试
        let batch_sigs: Vec<RpcConfirmedTransactionStatusWithSignature> = match JSON_RPC_CLIENT
            .get_signatures_for_address_with_config(address, config)
            .await
        {
            Ok(res) => res,
            Err(e) => {
                eprintln!("❌ RPC 查询签名失败，重试中 (错误: {:?})", e);
                tokio::time::sleep(Duration::from_secs(2)).await; // 注意：如果用的是 async-std，保留 time::sleep，否则换成 tokio::time
                continue;
            }
        };

        // 批次为空，说明已到历史末尾，退出循环
        if batch_sigs.is_empty() {
            println!("已到达历史记录的末尾。");
            break;
        }

        // 记录当前批次最旧的签名（用于下一次分页）
        let mut oldest_signature_in_batch: Option<Signature> = None;

        // 遍历当前批次的签名信息，提取签名
        for sig_info in batch_sigs {
            // 解析签名字符串为 Signature 类型
            let sig = match Signature::from_str(&sig_info.signature) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("⚠️ 签名解析失败：{}，跳过", e);
                    continue;
                }
            };

            // 更新当前批次最旧的签名（最后一个元素就是最旧的）
            oldest_signature_in_batch = Some(sig.clone());

            // 将签名添加到结果列表
            signatures.push(sig);

            // 如果设置了最大条数，且已达到，直接退出循环
            if let Some(max) = max_count
                && signatures.len() >= max
            {
                // 截断到最大条数
                signatures.truncate(max);
                // 标记为已达到上限，直接退出所有循环
                oldest_signature_in_batch = None;
                break;
            }
        }

        // 如果达到最大条数，退出外层循环
        if let Some(max) = max_count
            && signatures.len() >= max
        {
            break;
        }

        // 设置下一次查询的 before 参数
        before_signature = oldest_signature_in_batch;

        // 避免 RPC 请求过快，添加短暂休眠
        tokio::time::sleep(Duration::from_millis(100)).await; // 同上，注意时间库的选择
    }

    println!("✅ 共获取到 {} 条交易签名", signatures.len());
    Ok(signatures)
}

#[tokio::test]
async fn test_fetch_tx() {
    use dotenvy::dotenv;
    use env_logger::Builder;
    use env_logger::fmt::Formatter;
    use std::io::Write;
    dotenv().ok();
    Builder::new()
        .format(|buf: &mut Formatter, record: &log::Record| {
            let ts = buf.timestamp_micros();
            writeln!(
                buf,
                "[{} {} {}] {}",
                ts,
                record.level(),
                record.target(),
                record.args()
            )
        })
        .filter_level(log::LevelFilter::Info) // 关键：设置默认级别
        .init();
    use std::str::FromStr;
    println!("{:#?}",get_tx(&Signature::from_str("2T3MH4NS7odnBKf7H9N2MQpNg7z4uqVdrd8wsqNxSuVAx6ndWE8XYHkQFxQjMX2EtH4UExohFFLq49Rh35G1R6Yn").unwrap()).await);
}

#[tokio::test]
async fn test_fetch_user() {
    use dotenvy::dotenv;
    use env_logger::Builder;
    use env_logger::fmt::Formatter;
    use std::io::Write;
    dotenv().ok();
    Builder::new()
        .format(|buf: &mut Formatter, record: &log::Record| {
            let ts = buf.timestamp_micros();
            writeln!(
                buf,
                "[{} {} {}] {}",
                ts,
                record.level(),
                record.target(),
                record.args()
            )
        })
        .filter_level(log::LevelFilter::Info) // 关键：设置默认级别
        .init();
    use std::str::FromStr;
    println!(
        "{:#?}",
        tx_for_address(
            &Pubkey::from_str("dshAybqFXYVVTd4mzy9Uk6KD7km8wE9iZgPMYZdzEXc").unwrap(),
            Some(100)
        )
        .await
    );
}

pub async fn get_slot(slot: u64) -> anyhow::Result<UiConfirmedBlock> {
    let config = solana_client::rpc_config::RpcBlockConfig {
        encoding: UiTransactionEncoding::Json.into(),
        transaction_details: TransactionDetails::Signatures.into(),
        rewards: Some(true),
        commitment: CommitmentConfig::confirmed().into(),
        max_supported_transaction_version: Some(0),
    };
    let block = JSON_RPC_CLIENT.get_block_with_config(slot, config).await?;
    Ok(block)
}
