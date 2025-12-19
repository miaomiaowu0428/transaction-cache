use log::info;
use serde::{Deserialize, Serialize};
use solana_client::rpc_config::{RpcTransactionConfig, UiTransactionEncoding};
use solana_sdk::signature::Signature;
use solana_transaction_status_client_types::{
    EncodedConfirmedTransactionWithStatusMeta, EncodedTransactionWithStatusMeta,
};
use std::path::Path;
use std::str::FromStr;
use std::{collections::HashMap, sync::LazyLock};
use tokio::fs;
use tokio::sync::RwLock;
use utils::JSON_RPC_CLIENT;

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
        return Ok(Some(detail.clone()));
    }
    drop(cache);
    // fetch with retry
    info!("feching :{}",sig);
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
}

/// 持久化缓存到本地
pub async fn save_cache() -> anyhow::Result<()> {
    let cache = TX_DETAIL_CACHE.read().await;
    let map: HashMap<String, &TxDetail> = cache.iter().map(|(k, v)| (k.to_string(), v)).collect();
    let content = serde_json::to_string_pretty(&map)?;
    fs::write(LOCAL_CACHE_PATH, content).await?;
    Ok(())
}

#[tokio::test]
async fn test_fetch() {
    println!("{:#?}",get_tx(&Signature::from_str("2T3MH4NS7odnBKf7H9N2MQpNg7z4uqVdrd8wsqNxSuVAx6ndWE8XYHkQFxQjMX2EtH4UExohFFLq49Rh35G1R6Yn").unwrap()).await);
}
