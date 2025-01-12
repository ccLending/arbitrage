#![allow(unused)]

use solana_sdk::{
    pubkey::Pubkey,
    instruction::Instruction,
    account::Account,
    commitment_config::CommitmentConfig,
    sysvar::clock::Clock,
};
use solana_client::{
    rpc_client::RpcClient,
    rpc_config::RpcSendTransactionConfig,
};
use anchor_client::{Cluster, Program};
use anchor_client::anchor_lang::AccountDeserialize;
use std::{collections::HashMap, fmt::Debug, error::Error, str::FromStr};
use raydium_library::common;
use crate::pool::PoolOperations;

use meteora_dlmm_sdk::quote::{
    SwapExactInQuote, quote_exact_in, get_bin_array_pubkeys_for_swap,
};
use meteora_dlmm::state::{
    lb_pair::LbPair, bin::BinArray, bin_array_bitmap_extension::BinArrayBitmapExtension, 
};

pub const METEORA_DLMM_PROGRAM_ID: &str = "LBUZKhRxPF3XUpBCjp4YzTKgLccjZhTSDM9YuVaPwxo";

#[derive(Debug)]
pub struct MeteoraPool {
    pub pool_id: Pubkey,
    pub lb_pair: LbPair,
    pub bitmap_extension_key: Pubkey,
    pub bitmap_extension: Option<BinArrayBitmapExtension>,
    //pub left_bin_array_pubkeys: Vec<Pubkey>,
    //pub right_bin_array_pubkeys: Vec<Pubkey>,
    pub bin_arrays: HashMap<Pubkey, BinArray>,
    pub clock: Clock,
}

impl PoolOperations for MeteoraPool {
    fn calc_quote(
        &self,
        a_to_b: bool,
        amount_in: u64,
    ) -> u64 {
        match quote_exact_in(
            self.pool_id,
            &self.lb_pair,
            amount_in,
            a_to_b,
            self.bin_arrays.clone(),
            self.bitmap_extension.as_ref(),
            self.clock.unix_timestamp as u64,
            self.clock.slot,
        ) {
            Ok(quote) => quote.amount_out,
            Err(e) => {
                //eprintln!("meteora {} : {}", self.get_pool_id(), e);
                0
            }
        }
    }

    fn get_mints(&self) -> Vec<Pubkey> {
        vec![self.lb_pair.token_x_mint, self.lb_pair.token_y_mint]
    }

    fn get_pool_id(&self) -> Pubkey {
        self.pool_id
    }
}
