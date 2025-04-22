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
#[serde(rename_all = "camelCase")]
struct Token {
    pub mint: String,
    pub symbol: String,
}

#[derive(Deserialize, Debug)]
struct Stats {
    pub day: f64,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct OrcaRawData {
    pub address: String,
    pub token_a: Token,
    pub token_b: Token,
    pub volume: Option<Stats>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct OrcaUrlData {
    pub whirlpools: Vec<OrcaRawData>,
}

pub async fn process_data() -> Result<(), reqwest::Error> {
    //let raw = File::open("orca-data-raw.json")?;
    //let dataset: Vec<OrcaRawData> = serde_json::from_reader(raw)?;
    let url = "https://api.mainnet.orca.so/v1/whirlpool/list";
    let response: OrcaUrlData = reqwest::get(url).await?.json().await?;
    
    let mut buffer = Vec::new();
    let mut mint_set = HashSet::new();
    for data in response.whirlpools {
        if let Some(volume) = data.volume {
            if volume.day >= VOLUME_THRESHOLD {
                buffer.push(PairData {
                    pair_name: format!("{}-{}", data.token_a.symbol, data.token_b.symbol),
                    pool_type: PoolType::Orca,
                    pool_id: Pubkey::from_str(&data.address).unwrap(),
                    token_mint_a: Pubkey::from_str(&data.token_a.mint).unwrap(),
                    token_mint_b: Pubkey::from_str(&data.token_b.mint).unwrap(),
                });
                mint_set.insert(data.token_a.mint);
                mint_set.insert(data.token_b.mint);
            } else {
                break;
            }
        }
    }
    println!("orca data len: {}", buffer.len());
    println!("orca mint set len: {}", mint_set.len());

    let out_file = File::create("orca-data.json").unwrap();
    let mut writer = BufWriter::new(out_file);
    serde_json::to_writer(&mut writer, &buffer);
    writer.flush();
    
    let mint_file = File::create("orca-mint-set.json").unwrap();
    let mut mint_writer = BufWriter::new(mint_file);
    let mint_vec = mint_set.into_iter().collect::<Vec<_>>();
    serde_json::to_writer(&mut mint_writer, &mint_vec);
    mint_writer.flush();
  
    Ok(())
}
