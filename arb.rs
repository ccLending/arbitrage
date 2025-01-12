use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
    instruction::Instruction,
};
use std::{
    str::FromStr, rc::Rc, collections::HashSet,
};
use crate::pool::*;

pub struct Arbitrager {
    pub token_mints: Vec<Pubkey>,
    pub graph_edges: Vec<HashSet<usize>>,
    pub graph: PoolGraph,
}

impl Arbitrager {
    pub fn brute_force_search(
        &self,
        start_mint_idx: usize,
        init_balance: u64,
        curr_balance: u64,
        path: Vec<usize>,
        pool_path: Vec<Rc<Box<dyn PoolOperations>>>,
        error_pools: &mut HashSet<Pubkey>,
    ) {
        if path.len() == 4 {
            return;
        };
        let src_curr = path[path.len() - 1];
        let src_mint = self.token_mints[src_curr];
        let edges = &self.graph_edges[src_curr];
        //edges hashset中只有一个值{0} 应该去掉。并且pool_len=1

        for &dst_mint_idx in edges {
            if path.len() == 3 && dst_mint_idx != start_mint_idx {
                continue;
            }
            if path.contains(&dst_mint_idx) && dst_mint_idx != start_mint_idx {
                continue;
            }
            
            let pools = self.graph.0
                .get(&src_curr)
                .unwrap().0
                .get(&dst_mint_idx)
                .unwrap();        
            for pool in pools {
                let pool_id = pool.get_pool_id(); 
                //println!("pool_id: {}", pool_id);
                if error_pools.contains(&pool_id) {
                    continue;
                }
                if let Some(_) = pool_path.iter().find(|pool| pool.get_pool_id() == pool_id) {
                    continue;
                }
               
                let mut new_path = path.clone();
                new_path.push(dst_mint_idx);
                let mut new_pool_path = pool_path.clone();
                new_pool_path.push(pool.clone());
          
                let pool_mints = pool.get_mints();
                let a_to_b = pool_mints[0] == src_mint;
                let new_balance = pool.calc_quote(a_to_b, curr_balance);
                if new_balance == 0 {
                    error_pools.insert(pool_id);
                    continue;
                }

                if dst_mint_idx == start_mint_idx {
                    if new_balance > init_balance + 500_000 {
                        println!("found arbitrage: {} -> {}, profit: {}, path: {:?}, pool_path:{:?}", 
                            init_balance, new_balance, new_balance - init_balance, new_path,
                            new_pool_path.iter().map(|pool| pool.get_pool_id()).collect::<Vec<_>>());
                    }
                } else {
                    self.brute_force_search(
                        start_mint_idx,
                        init_balance,
                        new_balance,
                        new_path,
                        new_pool_path,
                        error_pools,
                    );
                }
            }
        }
    }
}