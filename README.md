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


## AkBot查询
```rust
/// 通过akbot查询目标交易前后的对手交易
/// 需要准备好akbot权限key
aktool_search(param)->AktoolResponse;

/// 返回结果类型如下，一般只需要关注data
#[derive(Debug, Deserialize)]
pub struct AktoolResponse {
    pub code: i32,

    #[serde(default)]
    pub data: Option<Vec<TradeRecord>>,
}

/// data的原始类型如下; 语义数值类型是String
/// 可以通过.into()将内部转换为数值类型
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
```


## 获取区块交易
```rust
/// 可通过如下方式获取整个slot; 并按index获取其中交易的signature; 后可通过上述方法解析交易内容
let slot = 394956236;
let slot_content = get_slot(slot).await.unwrap();
let sig = slot_content.tx_at(818).unwrap();
let tx = get_tx(&sig).await.unwrap().unwrap();
let ixs = parse_fetched_json(tx).await;
info!("{ixs:#?}");
```

## 交易获取升级版（暂未稳定）
```rust
/// 此方法可同时指定回溯时间和最大笔数，取先满足者
let Ok(sigs) = SignatureFetcherBuilder::for_address(target_address)
    .max_count(10000)
    .max_age(Duration::from_secs(60*60))
    .build()
    .fetch().await else {
        panic!("fail to fetch tx sigs for target")
    };
```