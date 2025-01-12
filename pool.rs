#![allow(unused)]

use solana_sdk::{
    pubkey::Pubkey,
    account::Account,
    instruction::Instruction,
};
use anchor_client::{Cluster, Program};
use std::{
    str::FromStr, fmt::Debug, 
    rc::Rc, cell::RefCell,
    collections::HashMap,
};

pub trait PoolOperations: Debug {
    fn calc_quote(
        &self,
        a_to_b: bool,
        amount_in: u64,
    ) -> u64;

    fn get_mints(&self) -> Vec<Pubkey>;
    fn get_pool_id(&self) -> Pubkey; //test
    /*fn swap_ix(
        &self,
        program: &Program<C>,
        owner: &Pubkey,
        mint_in: &Pubkey,
        mint_out: &Pubkey,
    ) -> Vec<Instruction>;*/
}


#[derive(Debug, Clone)]
pub struct PoolEdge(pub HashMap<usize, Vec<Rc<Box<dyn PoolOperations>>>>);

#[derive(Debug)]
pub struct PoolGraph(pub HashMap<usize, PoolEdge>);

impl PoolGraph {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn add_pool(
        &mut self, 
        idx0: usize, 
        idx1: usize, 
        pool: Rc<Box<dyn PoolOperations>>,
    ) {
        let edges = self.0
            .entry(idx0)
            .or_insert_with(|| PoolEdge(HashMap::new()));
        let pools = edges.0
            .entry(idx1)
            .or_insert_with(|| vec![]);
        pools.push(pool);
    }
}