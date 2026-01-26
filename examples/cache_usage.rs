use anyhow::Result;
use dotenvy::dotenv;
use solana_sdk::signature::Signature;
use std::str::FromStr;
use transaction_cache::{get_slot, get_tx};

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    env_logger::init();

    println!("=== Transaction Cache 使用示例 ===\n");

    // 示例 1: 查询交易（第一次从 RPC 获取，第二次从缓存读取）
    println!("📦 示例 1: 查询交易缓存");
    let sig = Signature::from_str(
        "2T3MH4NS7odnBKf7H9N2MQpNg7z4uqVdrd8wsqNxSuVAx6ndWE8XYHkQFxQjMX2EtH4UExohFFLq49Rh35G1R6Yn",
    )?;

    println!("  第一次查询: {} (会从 RPC 获取)", sig);
    let tx1 = get_tx(&sig).await?;
    if let Some(tx) = &tx1 {
        println!("    ✅ Slot: {}, 成功: {}", tx.slot, tx.succeeded());
    }

    println!("\n  第二次查询: {} (从缓存读取)", sig);
    let tx2 = get_tx(&sig).await?;
    if let Some(tx) = &tx2 {
        println!(
            "    ✅ Slot: {}, 成功: {} (来自缓存)",
            tx.slot,
            tx.succeeded()
        );
    }

    // 示例 2: 查询 Slot 区块信息（第一次从 RPC 获取，第二次从缓存读取）
    println!("\n\n📦 示例 2: 查询 Slot 区块缓存");
    let slot = 250000000u64;

    println!("  第一次查询 slot: {} (会从 RPC 获取)", slot);
    let block1 = get_slot(slot).await?;
    println!(
        "    ✅ Blockhash: {}, 交易数: {}",
        block1.blockhash,
        block1.signatures.as_ref().map(|s| s.len()).unwrap_or(0)
    );

    println!("\n  第二次查询 slot: {} (从缓存读取)", slot);
    let block2 = get_slot(slot).await?;
    println!(
        "    ✅ Blockhash: {}, 交易数: {} (来自缓存)",
        block2.blockhash,
        block2.signatures.as_ref().map(|s| s.len()).unwrap_or(0)
    );

    println!("\n\n=== 缓存性能对比 ===");
    println!("  交易缓存:");
    println!("    - 第一次: 从 RPC 获取 (~500-2000ms)");
    println!("    - 第二次: 从 Sled 读取 (~1-10ms)");
    println!("    - 性能提升: 100-1000 倍");
    println!("\n  Slot 缓存:");
    println!("    - 第一次: 从 RPC 获取 (~200-1000ms)");
    println!("    - 第二次: 从 Sled 读取 (~1-5ms)");
    println!("    - 性能提升: 100-500 倍");

    println!("\n\n=== 存储位置 ===");
    println!("  数据库目录: ./tx_cache_db/");
    println!("  ├── transactions/  (交易缓存)");
    println!("  └── slots/         (区块缓存)");

    Ok(())
}
