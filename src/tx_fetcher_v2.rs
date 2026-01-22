use anyhow::Result;
use solana_client::rpc_client::GetConfirmedSignaturesForAddress2Config;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;
pub use utils::JSON_RPC_CLIENT;

use crate::get_tx;

pub struct SignatureFetcher {
    address: Pubkey,
    max_count: Option<usize>,
    max_age: Option<Duration>, // 改成 Duration
    batch_size: usize,
}

impl SignatureFetcher {
    pub async fn fetch(&self) -> Result<Vec<Signature>> {
        let mut signatures = Vec::new();
        let mut before: Option<Signature> = None;

        let now = SystemTime::now();

        loop {
            let batch_sigs = get_sigs_rpc(self.address, self.batch_size, before).await?;
            if batch_sigs.is_empty() {
                break;
            }

            before = batch_sigs.last().cloned();

            if let Some(max_age) = self.max_age {
                let mut left = 0;
                let mut right = batch_sigs.len();

                while left < right {
                    let mid = (left + right) / 2;
                    if let Some(tx) = get_tx(&batch_sigs[mid]).await? {
                        if let Some(block_time) = tx.block_time {
                            let tx_time = UNIX_EPOCH + Duration::from_secs(block_time as u64);
                            if now.duration_since(tx_time).unwrap_or_default() <= max_age {
                                left = mid + 1; // 时间还没到
                            } else {
                                right = mid; // 超过限制
                            }
                        } else {
                            left = mid + 1; // 没有 block_time 当作未过期
                        }
                    } else {
                        left = mid + 1; // 取不到 tx 也继续往右
                    }
                }

                let slice_end = left.min(batch_sigs.len());
                signatures.extend_from_slice(&batch_sigs[..slice_end]);

                if slice_end < batch_sigs.len() {
                    return Ok(signatures);
                }
            } else {
                signatures.extend_from_slice(&batch_sigs);
            }

            if let Some(max) = self.max_count && signatures.len() >= max {
                signatures.truncate(max);
                return Ok(signatures);
            }

            sleep(Duration::from_millis(100)).await;
        }

        Ok(signatures)
    }
}

/// Builder
pub struct SignatureFetcherBuilder {
    address: Pubkey,
    max_count: Option<usize>,
    max_age: Option<Duration>,
    batch_size: usize,
}

impl SignatureFetcherBuilder {
    pub fn for_address(address: Pubkey) -> Self {
        Self {
            address,
            max_count: Some(1000),
            max_age: None,
            batch_size: 1000,
        }
    }
    
    pub fn max_count(mut self, count: usize) -> Self {
        self.max_count = Some(count);
        self
    }
    pub fn max_age(mut self, duration: Duration) -> Self {
        self.max_age = Some(duration);
        self
    }


    pub fn batch_size(mut self, size: usize) -> Self {
        self.batch_size = size;
        self
    }

    pub fn build(self) -> SignatureFetcher {
        SignatureFetcher {
            address: self.address,
            max_count: self.max_count,
            max_age: self.max_age,
            batch_size: self.batch_size,
        }
    }
}

/// 封装 RPC 调用获取一批 signature
async fn get_sigs_rpc(
    address: Pubkey,
    limit: usize,
    before: Option<Signature>,
) -> Result<Vec<Signature>> {
    use crate::JSON_RPC_CLIENT;
    use solana_client::rpc_response::RpcConfirmedTransactionStatusWithSignature; // 你的客户端

    let config = GetConfirmedSignaturesForAddress2Config {
        limit: Some(limit),
        before,
        until: None,
        commitment: None,
    };

    let batch: Vec<RpcConfirmedTransactionStatusWithSignature> = JSON_RPC_CLIENT
        .get_signatures_for_address_with_config(&address, config)
        .await?;

    let mut sigs = Vec::with_capacity(batch.len());
    for sig_info in batch {
        if let Ok(sig) = Signature::from_str(&sig_info.signature) {
            sigs.push(sig);
        }
    }

    Ok(sigs)
}
