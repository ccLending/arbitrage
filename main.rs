#![allow(unused)]
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::{Keypair, Signer, read_keypair_file},
    system_instruction::create_account,
    transaction::Transaction,
    instruction::Instruction,
    sysvar::clock::{self, Clock},
};
use solana_client::{
    rpc_client::RpcClient,
    rpc_config::RpcSendTransactionConfig,
};
use spl_token::{
    instruction::{initialize_account, close_account},
};
use spl_associated_token_account::{
    get_associated_token_address,
    instruction::create_associated_token_account,
};
use anchor_client::{Client, Cluster};
use anchor_client::anchor_lang::AccountDeserialize;
use serde::{Serialize, Deserialize};
use std::{
    str::FromStr, error::Error, fs::File, 
    rc::Rc, cell::RefCell, sync::Arc, 
    io::{BufWriter, Write},
    collections::{HashMap, HashSet},
    thread,
};
use arrayref::{array_ref, array_refs};
use rayon::prelude::*;

use raydium_library::{common, amm};
use raydium_amm::{
    math::Calculator, 
    state::{AmmInfo, Loadable},
};
use orca_whirlpools_core::{
    swap_quote_by_input_token, 
    ExactInSwapQuote, TransferFee, WhirlpoolFacade, TickFacade, TickArrayFacade, TickArrays, 
};
use whirlpool_cpi::state::{
    Tick, TickArray, Whirlpool, 
};
use meteora_dlmm::{
    state::{lb_pair::LbPair, bin::BinArray, bin_array_bitmap_extension::BinArrayBitmapExtension},
    utils::pda::*,
};
use meteora_dlmm_sdk::quote::get_bin_array_pubkeys_for_swap;

#[path = "/home/ubuntu/orca-reader/src/comm.rs"] pub mod comm;
pub mod pool;
pub mod orca_pool;
pub mod meteora_pool;
pub mod ray_amm_pool;
pub mod arb;
use comm::{PairData, PoolType};
use crate::{
    pool::*, orca_pool::*, meteora_pool::*, ray_amm_pool::*, arb::*,
};

fn main() -> Result<(), Box<dyn Error>> {
    let rpc_client: Arc<RpcClient> = Arc::new(RpcClient::new("https://solana-mainnet.g.alchemy.com/v2/kbsd9kKHQW0TkUSqqtBvvOWyZvgDtD0i".to_string())); //这个最快
    //let rpc_client: Arc<RpcClient> = Arc::new(RpcClient::new("https://mainnet.helius-rpc.com?api-key=70a8384f-66bc-4e6e-8264-09c21e66cdeb".to_string())); 
    /*let wallet_keypair = read_keypair_file("/home/ubuntu/.config/solana/id.json")?;
    let payer_pubkey = wallet_keypair.pubkey();
    let payer = Arc::new(wallet_keypair);*/
    
    let rpc_meteora = Arc::clone(&rpc_client);
    let handle_meteora = thread::spawn(move || -> Vec<MeteoraPool> { 
        let meteora_program_id = Pubkey::from_str(METEORA_DLMM_PROGRAM_ID).unwrap(); 
        let meteora_pairs: Vec<PairData> = serde_json::from_reader(File::open("/home/ubuntu/orca-reader/meteora-data.json").unwrap()).unwrap();
        let meteora_pool_ids = meteora_pairs
            .iter()
            .map(|pair| pair.pool_id)
            .collect::<Vec<Pubkey>>();
        
        println!("fetch meteora pools data ...");
        let meteora_pools_data: Vec<LbPair> = meteora_pool_ids
            .par_chunks(100) 
            .flat_map(|chunk| {
                rpc_meteora.get_multiple_accounts_with_commitment(chunk, CommitmentConfig::processed())
                    .unwrap()
                    .value
                    .iter()
                    .map(|account| common::deserialize_anchor_account::<LbPair>(&account.as_ref().unwrap()).unwrap())
                    .collect::<Vec<_>>()
            })
            .collect();
        println!("finished meteora pools len: {}", meteora_pools_data.len());

        let bitmap_extension_keys = meteora_pool_ids
            .iter()
            .map(|pool_id| {
                let (bitmap_extension_key, _bump) = derive_bin_array_bitmap_extension(*pool_id);
                bitmap_extension_key
            })
            .collect::<Vec<Pubkey>>();

        let meteora_bin_array_keys = meteora_pools_data
            .iter()
            .zip(meteora_pool_ids.iter())
            .map(|(lb_pair, &pool_id)| {
                //发送交易指令时可能还要用到left/right
                let left_bin_array_pubkeys = 
                    get_bin_array_pubkeys_for_swap(pool_id, lb_pair, None, true, 1).unwrap();
                let right_bin_array_pubkeys =
                    get_bin_array_pubkeys_for_swap(pool_id, lb_pair, None, false, 1).unwrap();
                left_bin_array_pubkeys
                    .into_iter()
                    .chain(right_bin_array_pubkeys.into_iter())
                    .collect::<HashSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<Vec<Pubkey>>>();
        
        println!("fetch meteora bin arrays ...");
        let meteora_bin_arrays: Vec<Option<BinArray>> = meteora_bin_array_keys.concat()
            .par_chunks(100) 
            .flat_map(|chunk| {
                rpc_meteora.get_multiple_accounts_with_commitment(chunk, CommitmentConfig::processed())
                    .unwrap()
                    .value
                    .iter()
                    .map(|account| {
                        match account {
                            Some(account) => common::deserialize_anchor_account::<BinArray>(&account).ok(),
                            None => None,
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        println!("finished meteora bin arrays. total_len {}, valid_len {}", meteora_bin_arrays.len(), meteora_bin_arrays.iter().filter_map(|&a|a).count());

        let meteora_bin_arrays_group: Vec<Vec<Option<BinArray>>> = meteora_bin_array_keys
            .iter()
            .scan(0, |start, group| {
                let len = group.len();
                let chunk = meteora_bin_arrays[*start..*start + len].to_vec();
                *start += len;
                Some(chunk)
            })
            .collect();
        let meteora_bin_arrays_maps = meteora_bin_array_keys.into_iter()
            .zip(meteora_bin_arrays_group.into_iter())
            .map(|(keys, bin_arrays)| {
                keys.into_iter()
                    .zip(bin_arrays.into_iter())
                    .filter_map(|(key, bin_array)| {
                        if let Some(bin_array) = bin_array {
                            Some((key, bin_array))
                        } else {
                            None
                        }
                    })
                    .collect::<HashMap<_, _>>()
            })
            .collect::<Vec<_>>();

        //这块有没有更好的方式
        let clock_account = rpc_meteora.get_account(&clock::ID).unwrap();
        let clock: Clock = bincode::deserialize(&clock_account.data).unwrap();  

        meteora_pool_ids.into_iter()
            .zip(meteora_pools_data.into_iter())
            .zip(bitmap_extension_keys.into_iter())
            .zip(meteora_bin_arrays_maps.into_iter())
            .filter_map(|(((pool_id, lb_pair), bitmap_extension_key), bin_arrays)| {
                if !bin_arrays.is_empty() {
                    Some(MeteoraPool {
                        pool_id,
                        lb_pair,
                        bitmap_extension_key,
                        bitmap_extension: None,
                        bin_arrays,
                        clock: clock.clone(),
                    })
                } else {
                    None
                }
            })
            .collect()
    });
    
    //------------------------------------------------------------------------------------
    let rpc_orca = Arc::clone(&rpc_client);
    let handle_orca = thread::spawn(move || -> Vec<OrcaPool> { 
        let orca_program_id = Pubkey::from_str(ORCA_WHIRLPOOL_PROGRAM_ID).unwrap();
        let orca_pairs: Vec<PairData> = serde_json::from_reader(File::open("/home/ubuntu/orca-reader/orca-data.json").unwrap()).unwrap();
        let orca_pool_ids = orca_pairs
            .iter()
            .map(|pair| pair.pool_id)
            .collect::<Vec<Pubkey>>();
        
        println!("fetch orca pools data ...");
        let orca_pools_data: Vec<Whirlpool> = orca_pool_ids
            .par_chunks(100) 
            .flat_map(|chunk| {
                rpc_orca.get_multiple_accounts_with_commitment(chunk, CommitmentConfig::processed())
                    .unwrap()
                    .value
                    .iter()
                    .map(|account| common::deserialize_anchor_account::<Whirlpool>(&account.as_ref().unwrap()).unwrap())
                    .collect::<Vec<_>>()
            })
            .collect();
        println!("finished orca pools len: {}", orca_pools_data.len());

        let mut orca_tick_array_keys = orca_pools_data
            .iter()
            .zip(orca_pool_ids.iter())
            .map(|(pool, pool_id)| {
                get_tick_array_pubkeys(pool.tick_current_index, pool.tick_spacing, true, &orca_program_id, pool_id)[0]  //目前只取一个
            })
            .collect::<Vec<Pubkey>>();
        let orca_tick_array_keys_b_a = orca_pools_data
            .iter()
            .zip(orca_pool_ids.iter())
            .map(|(pool, pool_id)| {
                get_tick_array_pubkeys(pool.tick_current_index, pool.tick_spacing, false, &orca_program_id, pool_id)[0]
            })
            .collect::<Vec<Pubkey>>();
        let diff_keys = orca_tick_array_keys
            .iter()
            .zip(orca_tick_array_keys_b_a.iter().enumerate())
            .filter_map(|(key, (pos, key_b_a))| {
            if key != key_b_a {
                Some((pos, *key_b_a))
            } else {
                None
            }
            })
            .collect::<Vec<_>>();
        orca_tick_array_keys.extend(diff_keys.iter().cloned().map(|(pos, key)| key).collect::<Vec<Pubkey>>());
        
        println!("fetch orca tick arrays ...");
        let mut orca_tick_arrays: Vec<Option<TickArray>> = orca_tick_array_keys
            .par_chunks(100) 
            .flat_map(|chunk| {
                rpc_orca.get_multiple_accounts_with_commitment(chunk, CommitmentConfig::processed())
                    .unwrap()
                    .value
                    .iter()
                    .map(|account| {
                        match account {
                            Some(account) => common::deserialize_anchor_account::<TickArray>(&account).ok(),
                            None => None,
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        println!("finished orca tick arrays. total_len {}, valid_len {}", orca_tick_arrays.len(), orca_tick_arrays.iter().filter_map(|&a|a).count());

        let orca_tick_arrays_diff: Vec<Option<TickArray>> = if diff_keys.len() > 0 {
            let original_len = orca_tick_array_keys.len() - diff_keys.len();
            orca_tick_array_keys.drain(original_len..);
            orca_tick_arrays.drain(original_len..).collect()
        } else {
            vec![]
        };

        let mut orca_pools: Vec<OrcaPool> = orca_pool_ids.into_iter()
            .zip(orca_pools_data.into_iter())
            .zip(orca_tick_array_keys.into_iter())
            .zip(orca_tick_arrays.into_iter())
            .map(|(((pool_id, pool), tick_array_key), tick_array)| {
                OrcaPool {
                    pool_id,
                    tick_current_index: pool.tick_current_index,
                    tick_spacing: pool.tick_spacing,
                    fee_rate: pool.fee_rate,
                    protocol_fee_rate: pool.protocol_fee_rate,
                    liquidity: pool.liquidity,
                    sqrt_price: pool.sqrt_price,
                    token_mint_a: pool.token_mint_a,
                    token_mint_b: pool.token_mint_b,
                    token_vault_a: pool.token_vault_a,
                    token_vault_b: pool.token_vault_b,
                    tick_array_key,
                    tick_array: tick_array,
                    tick_array_key_b_a: None,
                    tick_array_b_a: None,
                }
            })
            .collect();

        if diff_keys.len() > 0 {
            diff_keys.into_iter()
                .zip(orca_tick_arrays_diff.into_iter())
                .for_each(|((pos, key), tick_array)| {
                    orca_pools[pos].tick_array_key_b_a = Some(key);
                    orca_pools[pos].tick_array_b_a = tick_array;
                });
        }
        orca_pools.retain(|pool| pool.tick_array.is_some());
        orca_pools
    });

    //------------------------------------------------------------------------------------
    
    let rpc_rayamm = Arc::clone(&rpc_client);
    let handle_rayamm = thread::spawn(move || -> Vec<RayAmmPool> { 
        let ray_amm_program_id = Pubkey::from_str(RAY_AMM_PROGRAM_ID).unwrap();
        let ray_amm_pairs: Vec<PairData> = serde_json::from_reader(File::open("/home/ubuntu/orca-reader/raydium-amm-data.json").unwrap()).unwrap();
        let ray_amm_pool_ids = ray_amm_pairs
            .iter()
            .map(|pair| pair.pool_id)
            .collect::<Vec<Pubkey>>();
        
        println!("fetch ray amm pools data ...");
        let amm_size = std::mem::size_of::<AmmInfo>();
        let ray_amm_infos: Vec<Option<AmmInfo>> = ray_amm_pool_ids
            .par_chunks(100) 
            .flat_map(|chunk| {
                rpc_rayamm.get_multiple_accounts_with_commitment(chunk, CommitmentConfig::processed())
                    .unwrap()
                    .value
                    .iter()
                    .map(|account| {
                        match account {
                            Some(account) => {
                                if account.data.len() == amm_size {
                                    AmmInfo::load_from_bytes(&account.data).ok().cloned()
                                } else {
                                    None
                                }
                            }
                            None => None
                        }      
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        println!("finished ray amm pools len: {}, valid_len {}", ray_amm_infos.len(), ray_amm_infos.iter().filter_map(|&a|a).count());

        let ray_amm_pools_data = ray_amm_pool_ids
            .into_iter()
            .zip(ray_amm_infos.into_iter())
            .filter_map(|(pool_id, amm_info)| {
                if let Some(amm_info) = amm_info {
                    Some((pool_id, amm_info))
                } else {
                    None
                }
            })
            .collect::<Vec<(Pubkey, AmmInfo)>>();
        
        let ray_amm_vault_keys = ray_amm_pools_data
            .iter()
            .map(|(_, amm_info)| vec![amm_info.coin_vault, amm_info.pc_vault])
            .collect::<Vec<_>>();
        
        println!("fetch ray amm vaults amount ...");
        let ray_amm_vaults_amount: Vec<u64> = ray_amm_vault_keys.concat()
            .par_chunks(100) 
            .flat_map(|chunk| {
                rpc_rayamm.get_multiple_accounts_with_commitment(chunk, CommitmentConfig::processed())
                    .unwrap()
                    .value
                    .iter()
                    .map(|account| {
                        let vault = common::unpack_token(&account.as_ref().unwrap().data).unwrap();
                        vault.base.amount
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        println!("finished ray amm vaults amount: {}", ray_amm_vaults_amount.len());
        
        ray_amm_pools_data.into_iter()
            .zip(ray_amm_vaults_amount.chunks(2))
            .map(|((pool_id, amm_state), chunk)| {
                let (pc_vault_amount, coin_vault_amount) = 
                    Calculator::calc_total_without_take_pnl_no_orderbook(chunk[1], chunk[0], &amm_state).unwrap();
                RayAmmPool {
                    pool_id,
                    amm_state,
                    coin_vault_amount,
                    pc_vault_amount,
                }
            })
            .collect()  
    });

    let meteora_pools: Vec<MeteoraPool> = handle_meteora.join().unwrap();
    let orca_pools: Vec<OrcaPool> = handle_orca.join().unwrap();
    let ray_amm_pools: Vec<RayAmmPool> = handle_rayamm.join().unwrap();
    println!("meteora_pools_len: {}, orca_pools_len: {}, ray_amm_pools_len: {}", 
        meteora_pools.len(), orca_pools.len(), ray_amm_pools.len());
    
    let mut all_pools: Vec<Box<dyn PoolOperations>> = Vec::new();   
    all_pools.extend(meteora_pools
        .into_iter()
        .map(|pool| {
            let boxed_pool: Box<dyn PoolOperations> = Box::new(pool);
            boxed_pool
        })
        .collect::<Vec<_>>()
    );
    all_pools.extend(orca_pools
        .into_iter()
        .map(|pool| {
            let boxed_pool: Box<dyn PoolOperations> = Box::new(pool);
            boxed_pool
        })
        .collect::<Vec<_>>()
    );
    all_pools.extend(ray_amm_pools
        .into_iter()
        .map(|pool| {
            let boxed_pool: Box<dyn PoolOperations> = Box::new(pool);
            boxed_pool
        })
        .collect::<Vec<_>>()
    );

    let mut mint2idx = HashMap::new();
    let mut token_mints = vec![];
    let mut graph_edges = vec![];
    let mut graph = PoolGraph::new(); 

    //先把WSQL加到token_mints和mint2idx中，然后把all_ppols的顺序打乱。
    //统计一下遍历图一共用了多少步
    for pool in all_pools {
        let idxs = pool.get_mints()
            .into_iter()
            .map(|mint| 
                if let Some(&idx) = mint2idx.get(&mint) {
                    idx
                } else {
                    let idx = token_mints.len();
                    mint2idx.insert(mint, idx);
                    token_mints.push(mint);
                    graph_edges.push(HashSet::new());
                    idx
                }
            )
            .collect::<Vec<usize>>();
        
        let idx0 = idxs[0];
        let idx1 = idxs[1];
        if !graph_edges[idx0].contains(&idx1) {
            graph_edges[idx0].insert(idx1);
        }
        if !graph_edges[idx1].contains(&idx0) {
            graph_edges[idx1].insert(idx0);
        }

        let pool = Rc::new(pool);
        graph.add_pool(idx0, idx1, pool.clone());
        graph.add_pool(idx1, idx0, pool);
    }
  
    let total_count: usize = graph.0.values()
        .flat_map(|edges| edges.0.values())
        .map(|vec| vec.len())
        .sum();
    println!("graph total count {total_count}");

    let arbitrager = Arbitrager {
        token_mints,
        graph_edges,
        graph,
    };

    let start_mint = Pubkey::from_str("So11111111111111111111111111111111111111112")?;
    let start_mint_idx = *mint2idx.get(&start_mint).unwrap();
    let init_balance = 500_000000;
    let mut error_pools = HashSet::new();

    println!("start search arbitrage ... ");
    arbitrager.brute_force_search(
        start_mint_idx,
        init_balance,
        init_balance,
        vec![start_mint_idx],
        vec![],
        &mut error_pools,
    );

    println!("done");
    Ok(())
}