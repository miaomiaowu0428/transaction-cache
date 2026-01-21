use std::str::FromStr;

use solana_sdk::signature::Signature;
use solana_transaction_status_client_types::UiConfirmedBlock;

pub trait TxSequence {
    fn tx_at(&self, i: usize) -> Option<Signature>;
    fn tx_count(&self) -> usize;
}

impl TxSequence for UiConfirmedBlock {
    fn tx_at(&self, i: usize) -> Option<Signature> {
        self.signatures
            .as_ref()?
            .get(i)
            .and_then(|sig| Signature::from_str(sig).ok())
    }

    fn tx_count(&self) -> usize {
        self.signatures.as_ref().map(|v| v.len()).unwrap_or(0)
    }
}
