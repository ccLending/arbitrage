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
struct Mint {
    pub address: String,
    pub symbol: String,
    pub name: String,
    pub decimals: u8,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Stats {
    pub volume: f64,
    pub volume_quote: f64,
    pub volume_fee: f64,
    pub price_min: f64,
    pub price_max: f64,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct RaydiumAmmRawData {
    //pub r#type: String,
    pub id: String,
    pub mint_a: Mint,
    pub mint_b: Mint,
    pub mint_amount_a: f64,
    pub mint_amount_b: f64,
    pub day: Stats,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct RaydiumAmmOuterData {
    pub count: usize,
    pub data: Vec<RaydiumAmmRawData>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct RaydiumAmmUrlData {
    pub data: RaydiumAmmOuterData,
}

pub async fn process_data() -> Result<(), reqwest::Error> {
    let url = "https://api-v3.raydium.io/pools/info/list?poolType=standard&poolSortField=default&sortType=desc&pageSize=1000&page=1";
    let response: RaydiumAmmUrlData = reqwest::get(url).await?.json().await?;
    let url = "https://api-v3.raydium.io/pools/info/list?poolType=standard&poolSortField=default&sortType=desc&pageSize=1000&page=2";
    let response2: RaydiumAmmUrlData = reqwest::get(url).await?.json().await?;
    //let url = "https://api-v3.raydium.io/pools/info/list?poolType=standard&poolSortField=default&sortType=desc&pageSize=1000&page=3";
    //let response3: RaydiumAmmUrlData = reqwest::get(url).await?.json().await?;
    println!("ray amm 2000 volume: {}", response2.data.data[999].day.volume);

    let orca_mint_set: Vec<String> = serde_json::from_reader(File::open("/home/ubuntu/orca-reader/orca-mint-set.json").unwrap()).unwrap();
    let meteora_mint_set: Vec<String> = serde_json::from_reader(File::open("/home/ubuntu/orca-reader/meteora-mint-set.json").unwrap()).unwrap();
    //println!("orca mint set: {}, meteora mint set: {}", orca_mint_set.len(), meteora_mint_set.len());
    let mint_set: HashSet<String> = orca_mint_set.into_iter().chain(meteora_mint_set.into_iter()).collect();
    println!("orca & meterora union mint set {}", mint_set.len());

    let mut buffer = Vec::new();
    let mut pool_id_set = HashSet::new();
    for data in response.data.data.into_iter()
        .chain(response2.data.data.into_iter())
        //.chain(response3.data.data.into_iter()) 
    {
        /*if data.day.volume < VOLUME_THRESHOLD {
            break;
        }*/
        if mint_set.contains(&data.mint_a.address) && mint_set.contains(&data.mint_b.address) {
            if pool_id_set.insert(data.id.clone()) {
                buffer.push(PairData {
                    pair_name: format!("{}-{}", data.mint_a.symbol, data.mint_b.symbol),
                    pool_type: PoolType::RaydiumAmm,
                    pool_id: Pubkey::from_str(&data.id).unwrap(),
                    token_mint_a: Pubkey::from_str(&data.mint_a.address).unwrap(),
                    token_mint_b: Pubkey::from_str(&data.mint_b.address).unwrap(),
                });
            }
        }
    }
    println!("raydium amm data len: {}", buffer.len());

    let out_file = File::create("raydium-amm-data.json").unwrap();
    let mut writer = BufWriter::new(out_file);
    serde_json::to_writer(&mut writer, &buffer);
    writer.flush();
    Ok(())
}



/*let raw = File::open("ray-amm-raw-data.json")?;
    let mut dataset: Vec<RaydiumAmmRawData> = serde_json::from_reader(raw)?;
    let raw2 = File::open("ray-amm-raw-data2.json")?;
    let mut dataset2: Vec<RaydiumAmmRawData> = serde_json::from_reader(raw2)?;
    dataset.append(&mut dataset2);*/