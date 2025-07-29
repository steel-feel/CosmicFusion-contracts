use crate::error::ContractError;
use sha3::{Digest, Keccak256};
pub fn only_taker(
    current_time: u64,
    withdrawal: u64,
    dest_cancellation: u64,
) -> Result<(), ContractError> {
    if withdrawal > current_time {
        return Err(ContractError::DestWithrawTimeLimit);
    }
    // onlyBefore(immutables.timelocks.get(TimelocksLib.Stage.DstCancellation))
    if dest_cancellation < current_time {
        return Err(ContractError::DestCancelTimeLimit);
    }

    Ok(())
}

pub fn only_after(  current_time: u64, value: u64) -> bool  {
     value > current_time      
}

pub fn only_before(current_time: u64, value: u64) -> bool {
   value < current_time 
}

pub fn only_valid_secret(
    secret: String,
    hashlock: Vec<u8>
) -> Result<(), ContractError> {

    let mut hasher = Keccak256::new();
    hasher.update(secret.as_bytes());
    let computed_hash = hasher.finalize();

    if computed_hash.to_vec() != hashlock {
        return Err(ContractError::InvalidSecret);
    }

    Ok(())
}
