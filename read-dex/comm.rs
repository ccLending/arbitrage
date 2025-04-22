use serde::{Serialize, Deserialize};
use solana_sdk::pubkey::Pubkey;

pub const VOLUME_THRESHOLD: f64 = 100000.0;

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq)]
pub enum PoolType {
    RaydiumAmm,
    Orca,
    Meteora,
    //RaydiumClmm,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PairData {
    pub pair_name: String,
    pub pool_id: Pubkey,
    pub pool_type: PoolType,
    pub token_mint_a: Pubkey,
    pub token_mint_b: Pubkey,
    //pub volume: f64, 
}
