// use serde::{Deserialize, Serialize};
// use cosmwasm_schema::cw_serde;
use sylvia::cw_schema::cw_serde;
use sylvia::cw_std::{Addr, Coin, Uint256};

#[cw_serde(crate = "sylvia")]
pub struct Immutables {
    pub order_hash: Vec<u8>,
    pub hashlock: Vec<u8>,
    pub maker: Addr,
    pub taker: Addr,
    pub token: Coin,
    pub timelocks: Timelocks,
}

#[cw_serde(crate = "sylvia")]
pub struct Timelocks {
    pub withdrawal : Uint256,
    pub public_withdrawal : Uint256,
    pub dest_cancellation : Uint256,
    pub src_cancellation:Uint256,
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