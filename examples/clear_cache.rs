use anyhow::Result;
use sled;
use std::io::{self, Write};

fn main() -> Result<()> {
    println!("=== Transaction Cache 清理工具 ===\n");

    // 打开数据库
    let db = sled::open("tx_cache_db")?;

    // 打开各个 Tree
    let tx_tree = db.open_tree("transactions")?;
    let slot_tree = db.open_tree("slots")?;

    // 显示当前统计
    let tx_count = tx_tree.len();
    let slot_count = slot_tree.len();
    let disk_usage = db.size_on_disk()?;

    println!("当前缓存状态:");
    println!("  交易缓存: {} 条", tx_count);
    println!("  Slot 缓存: {} 条", slot_count);
    println!("  磁盘占用: {:.2} MB\n", disk_usage as f64 / 1_048_576.0);

    // 选择清理选项
    println!("请选择清理选项:");
    println!("  1. 清空交易缓存 (transactions)");
    println!("  2. 清空 Slot 缓存 (slots)");
    println!("  3. 清空所有缓存");
    println!("  4. 取消");
    print!("\n请输入选项 (1-4): ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let choice = input.trim();

    match choice {
        "1" => {
            print!("⚠️  确认清空交易缓存? 这将删除 {} 条记录 (y/N): ", tx_count);
            io::stdout().flush()?;
            let mut confirm = String::new();
            io::stdin().read_line(&mut confirm)?;

            if confirm.trim().to_lowercase() == "y" {
                tx_tree.clear()?;
                tx_tree.flush()?;
                println!("✅ 交易缓存已清空！");
            } else {
                println!("❌ 已取消");
            }
        }
        "2" => {
            print!(
                "⚠️  确认清空 Slot 缓存? 这将删除 {} 条记录 (y/N): ",
                slot_count
            );
            io::stdout().flush()?;
            let mut confirm = String::new();
            io::stdin().read_line(&mut confirm)?;

            if confirm.trim().to_lowercase() == "y" {
                slot_tree.clear()?;
                slot_tree.flush()?;
                println!("✅ Slot 缓存已清空！");
            } else {
                println!("❌ 已取消");
            }
        }
        "3" => {
            print!(
                "⚠️  确认清空所有缓存? 这将删除 {} 条记录 (y/N): ",
                tx_count + slot_count
            );
            io::stdout().flush()?;
            let mut confirm = String::new();
            io::stdin().read_line(&mut confirm)?;

            if confirm.trim().to_lowercase() == "y" {
                tx_tree.clear()?;
                slot_tree.clear()?;
                db.flush()?;
                println!("✅ 所有缓存已清空！");
            } else {
                println!("❌ 已取消");
            }
        }
        "4" => {
            println!("❌ 已取消");
        }
        _ => {
            println!("❌ 无效选项");
        }
    }

    // 显示清理后的状态
    println!("\n清理后状态:");
    println!("  交易缓存: {} 条", tx_tree.len());
    println!("  Slot 缓存: {} 条", slot_tree.len());
    println!(
        "  磁盘占用: {:.2} MB",
        db.size_on_disk()? as f64 / 1_048_576.0
    );

    Ok(())
}
