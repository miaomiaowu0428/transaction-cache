use log::info;
use serde::{Deserialize, Serialize};
use solana_client::rpc_client::GetConfirmedSignaturesForAddress2Config;
use solana_client::rpc_config::{CommitmentConfig, RpcTransactionConfig, UiTransactionEncoding};
use solana_client::rpc_response::RpcConfirmedTransactionStatusWithSignature;
use solana_sdk::signature::Signature;
use solana_transaction_status_client_types::{
    EncodedConfirmedTransactionWithStatusMeta, EncodedTransaction,
    EncodedTransactionWithStatusMeta, TransactionDetails, UiConfirmedBlock, UiMessage,
};
use std::str::FromStr;
use std::sync::LazyLock;
use std::time::Duration;
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

impl TxDetailLocal {
    pub fn succeeded(&self) -> bool {
        matches!(
            self.transaction.meta,
            Some(ref meta) if meta.err.is_none()
        )
    }
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

// 使用sled数据库作为存储后端
static DB: LazyLock<sled::Db> =
    LazyLock::new(|| sled::open("tx_cache_db").expect("Failed to open sled database"));

// 交易缓存 Tree
static TX_TREE: LazyLock<sled::Tree> = LazyLock::new(|| {
    DB.open_tree("transactions")
        .expect("Failed to open transactions tree")
});

// Slot 缓存 Tree
static SLOT_TREE: LazyLock<sled::Tree> =
    LazyLock::new(|| DB.open_tree("slots").expect("Failed to open slots tree"));

/// 通用：从 Tree 中获取数据（JSON 序列化）
fn get_from_tree<T: serde::de::DeserializeOwned>(
    tree: &sled::Tree,
    key: impl AsRef<[u8]>,
) -> anyhow::Result<Option<T>> {
    if let Some(bytes) = tree.get(key)? {
        let data: T = serde_json::from_slice(&bytes)?;
        Ok(Some(data))
    } else {
        Ok(None)
    }
}

/// 通用：保存数据到 Tree（JSON 序列化）
fn save_to_tree<T: serde::Serialize>(
    tree: &sled::Tree,
    key: impl AsRef<[u8]>,
    value: &T,
) -> anyhow::Result<()> {
    let bytes = serde_json::to_vec(value)?;
    tree.insert(key, bytes)?;
    tree.flush()?;
    Ok(())
}

/// 查询TxDetail，优先本地缓存，不存在则自动fetch（带重试）并写入缓存
pub async fn get_tx(sig: &Signature) -> anyhow::Result<Option<TxDetail>> {
    use solana_client::rpc_config::{RpcTransactionConfig, UiTransactionEncoding};
    use std::time::Duration;
    use tokio::time::sleep;

    // 先尝试从交易缓存读取
    let key = sig.to_string();
    if let Some(detail) = get_from_tree::<TxDetail>(&TX_TREE, &key)? {
        info!("found tx in cache: {sig}");
        return Ok(Some(detail));
    }

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
            // 保存到交易缓存
            save_to_tree(&TX_TREE, &key, &detail)?;
            Ok(Some(detail))
        } else {
            if let Some(e) = last_err {
                log::warn!("fetch {} error: {e}", sig);
            }
            Ok(None)
        }
    })
}

/// 持久化缓存到本地（sled数据库自动持久化，这个函数保留以兼容旧接口）
pub async fn save_cache() -> anyhow::Result<()> {
    // sled数据库会自动持久化，无需手动操作
    // 调用flush确保所有数据已写入磁盘
    TX_TREE.flush()?;
    SLOT_TREE.flush()?;
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
            before: before_signature,
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
            oldest_signature_in_batch = Some(sig);

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

#[tokio::test]
async fn test_fetch_slot() {
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
        .filter_level(log::LevelFilter::Info)
        .init();

    // 测试获取 slot（第一次会从 RPC 获取）
    let slot = 250000000u64;
    println!("第一次获取 slot {}...", slot);
    let block1 = get_slot(slot).await.unwrap();
    println!(
        "Slot {} 包含 {} 个签名",
        slot,
        block1.signatures.as_ref().map(|s| s.len()).unwrap_or(0)
    );

    // 第二次应该从缓存获取
    println!("\n第二次获取 slot {} (应该从缓存读取)...", slot);
    let block2 = get_slot(slot).await.unwrap();
    println!(
        "Slot {} 包含 {} 个签名",
        slot,
        block2.signatures.as_ref().map(|s| s.len()).unwrap_or(0)
    );

    assert_eq!(block1.blockhash, block2.blockhash);
}

/// 查询 Slot 区块信息，优先本地缓存，不存在则自动 fetch 并写入缓存
pub async fn get_slot(slot: u64) -> anyhow::Result<UiConfirmedBlock> {
    use std::time::Duration;
    use tokio::time::sleep;

    // 先尝试从 slot 缓存读取
    let key = slot.to_string();
    if let Some(block) = get_from_tree::<UiConfirmedBlock>(&SLOT_TREE, &key)? {
        info!("found slot {} in cache", slot);
        return Ok(block);
    }

    // 缓存未命中，从 RPC 获取
    info!("fetching slot {} from RPC", slot);
    let config = solana_client::rpc_config::RpcBlockConfig {
        encoding: UiTransactionEncoding::Json.into(),
        transaction_details: TransactionDetails::Signatures.into(),
        rewards: Some(true),
        commitment: CommitmentConfig::confirmed().into(),
        max_supported_transaction_version: Some(0),
    };

    // 带重试的 fetch 逻辑
    let mut retry_times = 3;
    let mut last_err = None;
    let mut fetched: Option<UiConfirmedBlock> = None;

    while retry_times > 0 {
        match JSON_RPC_CLIENT.get_block_with_config(slot, config).await {
            Ok(block) => {
                fetched = Some(block);
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

    if let Some(block) = fetched {
        // 保存到 slot 缓存
        save_to_tree(&SLOT_TREE, &key, &block)?;
        Ok(block)
    } else {
        Err(last_err.unwrap().into())
    }
}

pub trait GetAccounts {
    fn accounts(&self) -> Vec<Pubkey>;
}

impl GetAccounts for EncodedConfirmedTransactionWithStatusMeta {
    fn accounts(&self) -> Vec<Pubkey> {
        let EncodedTransaction::Json(tx) = &self.transaction.transaction else {
            return vec![];
        };
        let UiMessage::Raw(ui_tx) = &tx.message else {
            return vec![];
        };

        ui_tx
            .account_keys
            .iter()
            .map(|item| Pubkey::from_str(item).unwrap())
            .collect::<Vec<Pubkey>>()
    }
}

impl GetAccounts for TxDetailLocal {
    fn accounts(&self) -> Vec<Pubkey> {
        let EncodedTransaction::Json(tx) = &self.transaction.transaction else {
            return vec![];
        };
        let UiMessage::Raw(ui_tx) = &tx.message else {
            return vec![];
        };

        ui_tx
            .account_keys
            .iter()
            .map(|item| Pubkey::from_str(item).unwrap())
            .collect::<Vec<Pubkey>>()
    }
}
