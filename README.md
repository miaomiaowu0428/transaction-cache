## 本库内容

总共两个函数：

1. 获取某账户最近n笔交易的signature

```rust
pub async fn tx_for_address(address: &Pubkey, max_count: Option<usize>) -> Result<Vec<Signature>>;
```

2. 获取每个signature对应的具体交易内容

```rust
pub async fn get_tx(sig: &Signature) -> anyhow::Result<Option<TxDetail>>;
```

## 获取后用法：
### 使用 [solana-tx-parser](https://github.com/miaomiaowu0428/solana-tx-parser.git); [utils](https://github.com/miaomiaowu0428/sol-utils.git); 两个库进行重建与分析
1. 示例
```rust
// 获取到sig对应的内容
if let Some(res) = get_tx(&Signature::from_str(&sig.signature).unwrap()).await? {
    // 遍历筛选出的所有指令
    for transfer_ix in 
        // 将其展开为交易指令序列
        parse_fetched_json(res.into())
            .await
            .iter()
            // 筛选出转账指令; 转账指令类型需要预先定义
            .filter_map(|ix| Transfer::from_indexed_instruction(ix))
            .collect::<Vec<_>>()
    {
        // 分析操作
    }
}
```

### 需要预先定义好转账指令的结构: 
```rust
use borsh::{BorshDeserialize, BorshSerialize};
use solana_sdk::borsh1;
use solana_tx_parser::instruction;

// 来自solana_tx_parser的instruction宏
instruction!(
    // 转账指令是与system program交互
    program_id: "11111111111111111111111111111111",
    // 转账指令名称（可自定义）
    name: Transfer,
    // 转账指令的前4字节是固定的标识符
    discriminator: [0x02,0x00,0x00,0x00],
    // 转账指令的账户顺序定义
    accounts: {
        from: {
            writable: true,
            signer: true
        },
        to: {
            writable: true,
            signer: false
        }
    },
    // 转账指令的数据类型定义
    data: {
        lamports: u64,
    },
);

```