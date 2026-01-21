use solana_transaction_status_client_types::{
    EncodedConfirmedTransactionWithStatusMeta, EncodedTransactionWithStatusMeta, UiConfirmedBlock,
};

pub trait TxSequence {
    fn tx_at(&self, i: usize) -> Option<EncodedConfirmedTransactionWithStatusMeta>;
    fn tx_count(&self) -> usize;
}

impl TxSequence for (u64,UiConfirmedBlock) {
    fn tx_at(&self, i: usize) -> Option<EncodedConfirmedTransactionWithStatusMeta> {
        let tx = self.1.transactions.as_ref()?.get(i)?.clone();
        Some(EncodedConfirmedTransactionWithStatusMeta {
            slot: self.0,
            transaction: tx,
            block_time: self.1.block_time,
        })
    }

    fn tx_count(&self) -> usize {
        self.1.transactions.as_ref().map(|v| v.len()).unwrap_or(0)
    }
}
