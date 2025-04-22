mod orca;
mod meteora;
mod raydium_amm;
pub mod comm;

#[tokio::main]
async fn main() {
    match orca::process_data().await {
        Ok(_) => {}
        Err(e) => {
            eprintln!("{}", e);
        }
    }
    match meteora::process_data().await {
        Ok(_) => {}
        Err(e) => {
            eprintln!("{}", e);
        }
    }
    match raydium_amm::process_data().await {
        Ok(_) => {}
        Err(e) => {
            eprintln!("{}", e);
        }
    }
}