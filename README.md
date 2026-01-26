# Transaction Cache

Solana 交易缓存库，自动缓存 RPC 查询结果，避免重复请求。

## 核心功能

```rust
use transaction_cache::{get_tx, get_slot, tx_for_address};

// 1. 获取账户历史交易签名
let sigs = tx_for_address(&address, Some(100)).await?;

// 2. 获取交易详情（自动缓存）
let tx = get_tx(&sig).await?;

// 3. 获取区块信息（自动缓存）
let block = get_slot(slot).await?;
```

## 缓存机制

- **自动缓存**：首次 RPC 查询后自动保存，再次查询直接读缓存
- **性能提升**：100-1000 倍（RPC ~1000ms → 缓存 ~5ms）
- **持久化**：数据保存在 `./tx_cache_db/`，重启后仍可用
- **低内存**：按需加载，支持百万级记录

## 交易解析示例

配合 [solana-tx-parser](https://github.com/miaomiaowu0428/solana-tx-parser.git) 解析交易指令：

```rust
use solana_tx_parser::instruction;
use utils::parse_fetched_json;

// 定义指令结构
instruction!(
    program_id: "11111111111111111111111111111111",
    name: Transfer,
    discriminator: [0x02,0x00,0x00,0x00],
    accounts: {
        from: { writable: true, signer: true },
        to: { writable: true, signer: false }
    },
    data: { lamports: u64 }
);

// 解析交易
if let Some(tx) = get_tx(&sig).await? {
    for transfer in parse_fetched_json(tx.into()).await
        .iter()
        .filter_map(|ix| Transfer::from_indexed_instruction(ix))
    {
        println!("转账: {} lamports", transfer.lamports);
    }
}
```

## 区块查询示例

```rust
// 获取区块（自动缓存）
let block = get_slot(394956236).await?;

// 获取区块中的某个交易
let sig = block.tx_at(818)?;
let tx = get_tx(&sig).await?;
```

## 高级用法

### 按时间和数量过滤

```rust
use transaction_cache::tx_fetcher_v2::SignatureFetcherBuilder;

let sigs = SignatureFetcherBuilder::for_address(address)
    .max_count(10000)                        // 最多 10000 条
    .max_age(Duration::from_secs(3600))      // 最近 1 小时
    .build()
    .fetch()
    .await?;
```

### AkBot 对手盘查询

```rust
use transaction_cache::akbot::aktool_search;

let response = aktool_search(param).await?;
for trade in response.data.unwrap() {
    println!("{}: {} @ {}", trade.signature, trade.amount, trade.price);
}
```

## 实用工具

```bash
# 查看缓存统计
cargo run --example cache_stats

# 清理缓存
cargo run --example clear_cache
```
