#![allow(unused)]

use solana_sdk::pubkey::Pubkey;
use serde::{Serialize, Deserialize};
use std::{
    str::FromStr, collections::HashSet, fs::File, io::{BufWriter, Write},
};
use crate::comm::{
    VOLUME_THRESHOLD, PairData, PoolType,
};

#[derive(Deserialize, Debug)]
pub struct MeteoraRawData {
    pub address: String,
    pub name: String,
    pub mint_x: String,
    pub mint_y: String,
    pub trade_volume_24h: f64,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct MeteoraUrlData {
    pub pairs: Vec<MeteoraRawData>,
}

pub async fn process_data() -> Result<(), reqwest::Error> {
    let url = "https://dlmm-api.meteora.ag/pair/all_with_pagination?page=0&limit=500";
    let response: MeteoraUrlData = reqwest::get(url).await?.json().await?;

    let mut buffer = Vec::new();
    let mut mint_set = HashSet::new();
    for pair in response.pairs {
        if pair.trade_volume_24h >= VOLUME_THRESHOLD {
            buffer.push(PairData {
                pair_name: pair.name,
                pool_type: PoolType::Meteora,
                pool_id: Pubkey::from_str(&pair.address).unwrap(),
                token_mint_a: Pubkey::from_str(&pair.mint_x).unwrap(),
                token_mint_b: Pubkey::from_str(&pair.mint_y).unwrap(),
            });
            mint_set.insert(pair.mint_x);
            mint_set.insert(pair.mint_y);
        } else {
            break;
        }
    }
    println!("meteora data len: {}", buffer.len());
    println!("meteora mint set len: {}", mint_set.len());

    let out_file = File::create("meteora-data.json").unwrap();
    let mut writer = BufWriter::new(out_file);
    serde_json::to_writer(&mut writer, &buffer);
    writer.flush();

    let mint_file = File::create("meteora-mint-set.json").unwrap();
    let mut mint_writer = BufWriter::new(mint_file);
    let mint_vec = mint_set.into_iter().collect::<Vec<_>>();
    serde_json::to_writer(&mut mint_writer, &mint_vec);
    mint_writer.flush();

    Ok(())
}
