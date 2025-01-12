#![allow(unused)]

use solana_sdk::{
    pubkey::Pubkey,
    instruction::Instruction,
    account::Account,
};
use solana_client::{
    rpc_client::RpcClient,
    rpc_config::RpcSendTransactionConfig,
};
use anchor_client::{Cluster, Program};
use std::{collections::HashMap, fmt::Debug, error::Error, str::FromStr};

use crate::pool::PoolOperations;

use raydium_library::amm;
use raydium_amm::{
    math::{SwapDirection, Calculator}, 
    state::AmmInfo,
};

pub const RAY_AMM_PROGRAM_ID: &str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8"; 

#[derive(Debug)]
pub struct RayAmmPool {
    pub pool_id: Pubkey,
    pub amm_state: AmmInfo,
    pub coin_vault_amount: u64,
    pub pc_vault_amount: u64,
    //其中包括amm_authority需要计算，其他的都在amm_state中获取
    //let amm_keys = amm::load_amm_keys(&rpc_client, &ray_amm_program_id, &amm_pool)?; 
    //用于发送指令时，获取报价时不需要。获取较快。一次全部获取
    //let market_keys = amm::get_keys_for_market(rpc_client, &amm_keys.market_program, &amm_keys.market)?;
}

impl PoolOperations for RayAmmPool {
    fn calc_quote(
        &self,
        a_to_b: bool,
        amount_in: u64,
    ) -> u64 {
        let swap_direction = if a_to_b {
            SwapDirection::Coin2PC
        } else {
            SwapDirection::PC2Coin
        };
        let base_in = true;
        let slippage_bps = 0;
        
        match amm::swap_with_slippage(
            self.pc_vault_amount,
            self.coin_vault_amount,
            self.amm_state.fees.swap_fee_numerator,
            self.amm_state.fees.swap_fee_denominator,
            swap_direction,
            amount_in,
            base_in,
            slippage_bps,
        ) {
            Ok(quote) => quote,
            Err(e) => {
                eprintln!("ray amm {} : {}", self.get_pool_id(), e);
                0
            }
        }
    }

    fn get_mints(&self) -> Vec<Pubkey> {
        vec![self.amm_state.coin_vault_mint, self.amm_state.pc_vault_mint]
    }

    fn get_pool_id(&self) -> Pubkey {
        self.pool_id
    }
}
