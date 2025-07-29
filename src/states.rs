// use serde::{Deserialize, Serialize};
// use cosmwasm_schema::cw_serde;
use sylvia::cw_schema::cw_serde;
use sylvia::cw_std::{Addr, Coin};

#[cw_serde(crate = "sylvia")]
pub struct Immutables {
    pub order_hash: Vec<u8>,
    pub hashlock: Vec<u8>,
    pub maker: Addr,
    pub taker: Addr,
    pub token: Coin,
    pub timelocks: Timelocks,
}

//Values in seconds
#[cw_serde(crate = "sylvia")]
pub struct Timelocks {
    pub withdrawal: u64,
    pub public_withdrawal: u64,
    pub dest_cancellation: u64,
    pub src_cancellation: u64,
    pub src_withdrawal:u64,
}

#[cw_serde(crate = "sylvia")]
pub enum Stage {
    SrcWithdrawal,
    SrcPublicWithdrawal,
    SrcCancellation,
    SrcPublicCancellation,
    DstWithdrawal,
    DstPublicWithdrawal,
    DstCancellation,
}
