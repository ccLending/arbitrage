#![allow(unused)]

use solana_sdk::{
    pubkey::Pubkey,
    instruction::Instruction,
    account::Account,
    commitment_config::CommitmentConfig,
};
use solana_client::{
    rpc_client::RpcClient,
    rpc_config::RpcSendTransactionConfig,
};
use anchor_client::{Cluster, Program};

use orca_whirlpools_core::{
    swap_quote_by_input_token, 
    ExactInSwapQuote, TransferFee, WhirlpoolFacade, TickFacade, TickArrayFacade, TickArrays, 
};
use whirlpool_cpi::state::{
    Tick, TickArray, Whirlpool, 
};

use crate::pool::PoolOperations;

pub const ORCA_WHIRLPOOL_PROGRAM_ID: &str = "whirLbMiicVdio4qvUfM5KAg6Ct8VwpYzGff3uctyCc";
pub const TICK_ARRAY_COUNT: usize = 1; // 3

#[derive(Debug)]
pub struct OrcaPool {
    pub pool_id: Pubkey,
    pub tick_current_index: i32,
    pub tick_spacing: u16,
    pub fee_rate: u16, 
    pub protocol_fee_rate: u16,
    pub liquidity: u128,
    pub sqrt_price: u128,
    pub token_mint_a: Pubkey,
    pub token_mint_b: Pubkey,
    pub token_vault_a: Pubkey,
    pub token_vault_b: Pubkey,
    pub tick_array_key: Pubkey,
    pub tick_array: Option<TickArray>,
    pub tick_array_key_b_a: Option<Pubkey>,
    pub tick_array_b_a: Option<TickArray>,
}

impl OrcaPool {
    pub fn get_tick_array_facade(&self, a_to_b: bool) -> TickArrayFacade {
        let tick_array = if a_to_b  {
            self.tick_array.as_ref().unwrap()
        } else {
            if let Some(ref tick_array_b_a) = self.tick_array_b_a {
                tick_array_b_a
            } else {
                self.tick_array.as_ref().unwrap()
            }
        };
        let ticks: Vec<TickFacade> = tick_array.ticks
            .iter()
            .map(|tick| {
                TickFacade {
                    initialized: tick.initialized,
                    liquidity_net: tick.liquidity_net,
                    liquidity_gross: tick.liquidity_gross,
                    fee_growth_outside_a: tick.fee_growth_outside_a,
                    fee_growth_outside_b: tick.fee_growth_outside_b,
                    reward_growths_outside: [tick.reward_growths_outside[0], tick.reward_growths_outside[1], tick.reward_growths_outside[2]],
                }
            })
            .collect();
        TickArrayFacade {
            start_tick_index: tick_array.start_tick_index,
            ticks: ticks.try_into().unwrap(),
        }
    }
}

impl PoolOperations for OrcaPool {
    fn calc_quote(
        &self,
        a_to_b: bool,
        amount_in: u64,
    ) -> u64 {
        let tick_array_facade = self.get_tick_array_facade(a_to_b);
        let whrilpool_facade = WhirlpoolFacade {
            tick_spacing: self.tick_spacing,
            tick_current_index: self.tick_current_index,
            fee_rate: self.fee_rate,
            protocol_fee_rate: self.protocol_fee_rate,
            liquidity: self.liquidity,
            sqrt_price: self.sqrt_price,
            ..WhirlpoolFacade::default()
        };
        let slippage_bps = 0;
        let transfer_fee_a = None;
        let transfer_fee_b = None;

        match swap_quote_by_input_token(
            amount_in,
            a_to_b,
            slippage_bps,
            whrilpool_facade,
            TickArrays::One(tick_array_facade),
            transfer_fee_a,
            transfer_fee_b,
        ) {
            Ok(quote) => quote.token_est_out,
            Err(e) => {
                //eprintln!("orca {} : {}", self.get_pool_id(), e);
                0
            }
        } 
    }

    fn get_mints(&self) -> Vec<Pubkey> {
        vec![self.token_mint_a, self.token_mint_b]
    }

    fn get_pool_id(&self) -> Pubkey {
        self.pool_id
    }
}


const TICK_ARRAY_SIZE: i32 = 88;
const MIN_TICK_INDEX: i32 = -443636;
const MAX_TICK_INDEX: i32 = 443636;
const MIN_SQRT_PRICE: u128 = 4295048016;
const MAX_SQRT_PRICE: u128 = 79226673515401279992447579055;

fn floor_division(dividend: i32, divisor: i32) -> i32 {
    if dividend % divisor == 0 || dividend.signum() == divisor.signum() {
        dividend / divisor
    } else {
        dividend / divisor - 1
    }
}

fn get_default_sqrt_price_limit(a_to_b: bool) -> u128 {
    if a_to_b {
        MIN_SQRT_PRICE
    } else {
        MAX_SQRT_PRICE
    }
}

fn check_is_valid_start_tick(tick_index: i32, tick_spacing: i32) -> bool {
    let ticks_in_array = TICK_ARRAY_SIZE * tick_spacing;
    if !(MIN_TICK_INDEX..=MAX_TICK_INDEX).contains(&tick_index) {
        if tick_index > MIN_TICK_INDEX {
            return false;
        }
        let min_array_start_index =
            MIN_TICK_INDEX - (MIN_TICK_INDEX % ticks_in_array + ticks_in_array);
        return tick_index == min_array_start_index;
    }
    tick_index % ticks_in_array == 0
}

pub fn get_tick_array_pubkeys(
    tick_current_index: i32, 
    tick_spacing: u16, 
    a_to_b: bool,
    orca_program_id: &Pubkey,
    pool_id: &Pubkey,
) -> Vec<Pubkey> {
    let tick_spacing = tick_spacing as i32;
    let ticks_in_array = TICK_ARRAY_SIZE * tick_spacing;
    let start_tick_index_base = floor_division(tick_current_index, ticks_in_array) * ticks_in_array;
    let offset = if a_to_b {
        [0, -1, -2]
    } else {
        let shifted = tick_current_index + tick_spacing >= start_tick_index_base + ticks_in_array;
        if shifted {
            [1, 2, 3]
        } else {
            [0, 1, 2]
        }
    };    
    let start_tick_indexes = offset
        .iter()
        .filter_map(|&o| {
            let start_tick_index = start_tick_index_base + o * ticks_in_array;
            if check_is_valid_start_tick(start_tick_index, tick_spacing) {
                Some(start_tick_index)
            } else {
                None
            }
        })
        .collect::<Vec<i32>>();
    
    start_tick_indexes
        .iter()
        .take(TICK_ARRAY_COUNT) 
        .map(|start_tick_index| {
            Pubkey::find_program_address(
                &[
                    b"tick_array",
                    pool_id.as_ref(),
                    start_tick_index.to_string().as_bytes(),
                ],
                orca_program_id,
            ).0
        })
        .collect::<Vec<Pubkey>>()
}