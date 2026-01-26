use anyhow::Result;
use sled;

fn main() -> Result<()> {
    println!("=== Transaction Cache 统计信息 ===\n");

    // 打开数据库
    let db = sled::open("tx_cache_db")?;

    // 打开各个 Tree
    let tx_tree = db.open_tree("transactions")?;
    let slot_tree = db.open_tree("slots")?;

    // 统计交易缓存
    let tx_count = tx_tree.len();
    println!("📦 交易缓存 (transactions):");
    println!("   ├─ 记录数: {} 条", tx_count);

    // 统计 slot 缓存
    let slot_count = slot_tree.len();
    println!("\n📦 Slot 缓存 (slots):");
    println!("   ├─ 记录数: {} 条", slot_count);

    // 总磁盘占用
    let disk_usage = db.size_on_disk()?;
    println!("\n💾 磁盘占用:");
    println!(
        "   ├─ 总大小: {} bytes ({:.2} MB)",
        disk_usage,
        disk_usage as f64 / 1_048_576.0
    );

    // 估算平均大小
    if tx_count > 0 {
        let avg_tx_size = disk_usage / (tx_count + slot_count) as u64;
        println!(
            "   └─ 平均记录大小: {} bytes ({:.2} KB)",
            avg_tx_size,
            avg_tx_size as f64 / 1024.0
        );
    }

    // 可选：列出部分 key（仅前10个）
    println!("\n🔑 交易缓存样例 (前10条):");
    for (idx, item) in tx_tree.iter().enumerate() {
        if idx >= 10 {
            break;
        }
        let (key, _) = item?;
        println!("   {}: {}", idx + 1, String::from_utf8_lossy(&key));
    }

    println!("\n🔑 Slot 缓存样例 (前10条):");
    for (idx, item) in slot_tree.iter().enumerate() {
        if idx >= 10 {
            break;
        }
        let (key, _) = item?;
        println!("   {}: slot {}", idx + 1, String::from_utf8_lossy(&key));
    }

    println!("\n✅ 统计完成！");
    println!("\n提示：");
    println!("  - 如需清空交易缓存: tx_tree.clear()?");
    println!("  - 如需清空 slot 缓存: slot_tree.clear()?");
    println!("  - 数据库位置: ./tx_cache_db/");

    Ok(())
}
