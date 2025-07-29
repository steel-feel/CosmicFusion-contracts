use crate::error::ContractError;
use crate::states::{Immutables, Timelocks};
use cw_storage_plus::Item;
use sylvia::contract;

use crate::utils::{only_after, only_before, only_valid_secret};
use sylvia::ctx::{ExecCtx, InstantiateCtx, QueryCtx};
use sylvia::cw_schema::cw_serde;
#[cfg(not(feature = "library"))]
use sylvia::cw_std::Empty;
use sylvia::cw_std::{Addr, BankMsg, Coin, Response, SubMsg};
use sylvia::types::{CustomMsg, CustomQuery};

pub struct EscrowDest<E, Q> {
    pub rescue_delay: Item<u64>,
    pub immutables: Item<Immutables>,
    _phantom: std::marker::PhantomData<(E, Q)>,
}

#[cw_serde(crate = "sylvia::cw_schema")]
pub struct InstantiateMsgData {
    pub rescue_delay: u64,
    pub order_hash: String,
    pub hashlock: String,
    pub maker: Addr,
    pub taker: Addr,
    pub token: Coin,
    pub timelocks: Timelocks,
}

#[cw_serde(crate = "sylvia::cw_schema")]
pub struct WithdrawMsg {
    pub secret: String,
}

#[sylvia::cw_schema::cw_serde(crate = "sylvia::cw_schema")]
pub struct SvCustomMsg;
impl sylvia::cw_std::CustomMsg for SvCustomMsg {}

#[cfg_attr(not(feature = "library"), sylvia::entry_points(generics<Empty, Empty>))]
#[contract]
#[sv::error(ContractError)]
#[sv::custom(msg = E, query = Q)]
impl<E, Q> EscrowDest<E, Q>
where
    E: CustomMsg + 'static,
    Q: CustomQuery + 'static,
{
    //TODO: check if can pass anything in args
    pub const fn new() -> Self {
        Self {
            rescue_delay: Item::new("rescue_delay"),
            immutables: Item::new("immutables"),
            _phantom: std::marker::PhantomData,
        }
    }

    #[sv::msg(instantiate)]
    fn instantiate(
        &self,
        ctx: InstantiateCtx<Q>,
        data: InstantiateMsgData,
    ) -> Result<Response<E>, ContractError> {
        let mut ok = false;
        for asset in ctx.info.funds {
            if asset.denom == data.token.denom && asset.amount == data.token.amount {
                ok = true;
            }
        }
        if !ok {
            return Err(ContractError::UnmatchedDenomOrAmount);
        }

        let hashlock = hex::decode(&data.hashlock)
            .map_err(|e| sylvia::cw_std::StdError::generic_err(e.to_string()))?;

        let order_hash = hex::decode(&data.order_hash)
            .map_err(|e| sylvia::cw_std::StdError::generic_err(e.to_string()))?;

        self.rescue_delay
            .save(ctx.deps.storage, &data.rescue_delay)?;
        self.immutables.save(
            ctx.deps.storage,
            &Immutables {
                hashlock,
                order_hash,
                maker: data.maker,
                taker: data.taker,
                timelocks: data.timelocks,
                token: data.token,
            },
        )?;
        Ok(Response::new())
    }

    /// Withdraw function to be called by taker only
    #[sv::msg(exec)]
    fn withdraw(&self, ctx: ExecCtx<Q>, msg: WithdrawMsg) -> Result<Response<E>, ContractError> {
        let immutables = self.immutables.load(ctx.deps.storage)?;

        // Check if caller is taker
        if ctx.info.sender != immutables.taker {
            return Err(ContractError::OnlyTaker);
        }
        // Check timelock conditions
        let current_time_in_secs = ctx.env.block.time.seconds();
        if only_after(current_time_in_secs, immutables.timelocks.withdrawal) {
            return Err(ContractError::DestWithrawTimeLimit);
        }
        if only_before(current_time_in_secs, immutables.timelocks.dest_cancellation) {
            return Err(ContractError::DestCancelTimeLimit);
        }
        //Check secret hash
        only_valid_secret(msg.secret, immutables.hashlock)?;

        //send coins
        let msg = BankMsg::Send {
            to_address: immutables.maker.into(),
            amount: vec![immutables.token],
        };
        let submsg = SubMsg::reply_never(msg);

        Ok(Response::new().add_submessage(submsg))
    }

    #[sv::msg(exec)]
    fn public_withdraw(
        &self,
        ctx: ExecCtx<Q>,
        msg: WithdrawMsg,
    ) -> Result<Response<E>, ContractError> {
        let immutables = self.immutables.load(ctx.deps.storage)?;
        // Check timelock conditions
        let current_time_in_secs = ctx.env.block.time.seconds();

        if only_after(current_time_in_secs, immutables.timelocks.public_withdrawal) {
            return Err(ContractError::DestWithrawTimeLimit);
        }

        if only_before(current_time_in_secs, immutables.timelocks.dest_cancellation) {
            return Err(ContractError::DestCancelTimeLimit);
        }

        //Check secret hash
        only_valid_secret(msg.secret, immutables.hashlock)?;
        //send coins
        let msg = BankMsg::Send {
            to_address: immutables.maker.into(),
            amount: vec![immutables.token],
        };
        let submsg = SubMsg::reply_never(msg);

        Ok(Response::new().add_submessage(submsg))
    }


    #[sv::msg(exec)]
    fn cancel(&self, ctx: ExecCtx<Q> ) -> Result<Response<E>, ContractError> {
        let immutables = self.immutables.load(ctx.deps.storage)?;

        // Check if caller is taker
        if ctx.info.sender != immutables.taker {
            return Err(ContractError::OnlyTaker);
        }

        let current_time_in_secs = ctx.env.block.time.seconds();
        if only_after(current_time_in_secs, immutables.timelocks.dest_cancellation) {
            return Err(ContractError::DestCancelTimeLimit);
        }

        let msg = BankMsg::Send {
            to_address: immutables.taker.into(),
            amount: vec![immutables.token],
        };
        let submsg = SubMsg::reply_never(msg);

        Ok(Response::new().add_submessage(submsg))
    }

     #[sv::msg(exec)]
     fn rescue_funds(&self, ctx: ExecCtx<Q> ) -> Result<Response<E>, ContractError> { 
        let immutables = self.immutables.load(ctx.deps.storage)?;
        let rescue_delay = self.rescue_delay.load(ctx.deps.storage)?;
        // Check if caller is taker
        if ctx.info.sender != immutables.taker {
            return Err(ContractError::OnlyTaker);
        }

        let current_time_in_secs = ctx.env.block.time.seconds();
        let rescue_start = immutables.timelocks.src_withdrawal + rescue_delay;
        if only_after(current_time_in_secs, rescue_start) {
            return Err(ContractError::RescueTimeLimit);
        }

        let msg = BankMsg::Send {
            to_address: immutables.taker.into(),
            amount: vec![immutables.token],
        };
        let submsg = SubMsg::reply_never(msg);

        Ok(Response::new().add_submessage(submsg))
     }


    
    #[sv::msg(query)]
    fn get_order_hash(&self, ctx: QueryCtx<Q>) -> Result<OrderHashResponse, ContractError> {
        let imms = self.immutables.load(ctx.deps.storage)?;
        let order_hash = hex::encode(imms.order_hash);
        Ok(OrderHashResponse { order_hash })
    }

    #[sv::msg(query)]
    fn get_timelocks(&self, ctx: QueryCtx<Q>) -> Result<TimelockResponse, ContractError> {
        let imms = self.immutables.load(ctx.deps.storage)?;
        Ok(TimelockResponse {
            timelocks: imms.timelocks,
        })
    }

    #[sv::msg(query)]
    fn get_current_time(&self, ctx: QueryCtx<Q>) -> Result<CurrentTimeResponse, ContractError> {
        Ok(CurrentTimeResponse {
            time: ctx.env.block.time.seconds(),
        })
    }
}

#[cw_serde(crate = "sylvia")]
pub struct OrderHashResponse {
    pub order_hash: String,
}

#[cw_serde(crate = "sylvia")]
pub struct TimelockResponse {
    pub timelocks: Timelocks,
}

#[cw_serde(crate = "sylvia")]
pub struct CurrentTimeResponse {
    pub time: u64,
}

#[cfg(test)]
mod tests {
    use crate::states::Timelocks;

    use super::*;

    use sha3::{Digest, Keccak256};
    use sylvia::cw_multi_test::IntoAddr;
    use sylvia::cw_std::testing::{message_info, mock_dependencies, mock_env};
    use sylvia::cw_std::{Addr, Coin, Empty, Timestamp};

    // Unit tests don't have to use a testing framework for simple things.
    //
    // For more complex tests (particularly involving cross-contract calls), you
    // may want to check out `cw-multi-test`:
    // https://github.com/CosmWasm/cw-multi-test
    #[test]
    fn init() {
        let sender = "alice".into_addr();
        let contract = EscrowDest::<Empty, Empty>::new();
        let mut deps = mock_dependencies();
        let ctx = InstantiateCtx::from((
            deps.as_mut(),
            mock_env(),
            message_info(&sender, &[Coin::new(1000u32, "stake")]),
        ));

        let hashlock = {
            let mut hasher = Keccak256::new();
            hasher.update(b"secret");
            hex::encode(&hasher.finalize())
        };

        let order_hash = {
            let mut hasher = Keccak256::new();
            hasher.update(b"orderhash");
            hex::encode(&hasher.finalize()) //.to_ascii_lowercase()
        };

        let insta_data = InstantiateMsgData {
            rescue_delay: 1,
            hashlock,
            order_hash,
            maker: Addr::unchecked("maker"),
            taker: Addr::unchecked("taker"),
            timelocks: Timelocks {
                withdrawal: 1,
                public_withdrawal: 2,
                dest_cancellation: 3,
                src_cancellation: 4,
                src_withdrawal: 5,
            },
            token: Coin::new(1000u32, "stake"),
        };
        contract.instantiate(ctx, insta_data).unwrap();

        // We're inspecting the raw storage here, which is fine in unit tests. In
        // integration tests, you should not inspect the internal state like this,
        // but observe the external results.
        // assert_eq!(0, contract..load(deps.as_ref().storage).unwrap());
        assert_eq!(
           1, contract.rescue_delay.load(deps.as_ref().storage).unwrap()
        );
    }

    #[test]
    fn withdraw_only_by_taker() {
        let sender = "alice".into_addr();
        let contract = EscrowDest::<Empty, Empty>::new();
        let mut deps = mock_dependencies();
        let ctx = InstantiateCtx::from((
            deps.as_mut(),
            mock_env(),
            message_info(&sender, &[Coin::new(1000u32, "stake")]),
        ));
        let hashlock = {
            let mut hasher = Keccak256::new();
            hasher.update(b"secret");
            hex::encode(&hasher.finalize())
        };

        let order_hash = {
            let mut hasher = Keccak256::new();
            hasher.update(b"orderhash");
            hex::encode(&hasher.finalize()) //.to_ascii_lowercase()
        };

        println!("orderhash: {} \nhashlock: {}", order_hash, hashlock);

        let insta_data = InstantiateMsgData {
            rescue_delay: 1,
            hashlock,
            order_hash,
            maker: Addr::unchecked("maker"),
            taker: Addr::unchecked("taker"),
            timelocks: Timelocks {
                withdrawal: 1000,
                public_withdrawal: 2000,
                dest_cancellation: 3000,
                src_cancellation: 4000,
                src_withdrawal: 5000,
            },
            token: Coin::new(1000u32, "stake"),
        };
        contract.instantiate(ctx, insta_data).unwrap();

        let mut mock_env2 = mock_env();
        mock_env2.block.time = Timestamp::from_seconds(1500);

        let taker = Addr::unchecked("taker");
        let exe_ctx = ExecCtx::from((deps.as_mut(), mock_env2, message_info(&taker, &[])));

        contract
            .withdraw(
                exe_ctx,
                WithdrawMsg {
                    secret: String::from("secret"),
                },
            )
            .unwrap();
    }

    #[test]
    //#[should_panic = "Invalid Secret"]
    fn secret_does_not_match() {
        let sender = "alice".into_addr();
        let contract = EscrowDest::<Empty, Empty>::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((
            deps.as_mut(),
            mock_env(),
            message_info(&sender, &[Coin::new(1000u32, "stake")]),
        ));
        let hashlock = {
            let mut hasher = Keccak256::new();
            hasher.update(b"secret");
            hex::encode(&hasher.finalize())
        };

        let order_hash = {
            let mut hasher = Keccak256::new();
            hasher.update(b"orderhash");
            hex::encode(&hasher.finalize()) //.to_ascii_lowercase()
        };

        let insta_data = InstantiateMsgData {
            rescue_delay: 1,
            hashlock,
            order_hash,
            maker: Addr::unchecked("maker"),
            taker: Addr::unchecked("taker"),
            timelocks: Timelocks {
                withdrawal: 1000,
                public_withdrawal: 2000,
                dest_cancellation: 3000,
                src_cancellation: 4000,
                src_withdrawal: 5000,
            },
            token: Coin::new(1000u32, "stake"),
        };
        contract.instantiate(ctx, insta_data).unwrap();

        let mut mock_env2 = mock_env();
        mock_env2.block.time = Timestamp::from_seconds(1500);

        let taker = Addr::unchecked("taker");
        let exe_ctx = ExecCtx::from((deps.as_mut(), mock_env2, message_info(&taker, &[])));

        let err = contract
            .withdraw(
                exe_ctx,
                WithdrawMsg {
                    secret: String::from("wrong"),
                },
            )
            .unwrap_err();
        assert_eq!(err, ContractError::InvalidSecret);
    }

    #[test]
    fn query_timelocks() {
        let sender = "alice".into_addr();
        let contract = EscrowDest::<Empty, Empty>::new();
        let mut deps = mock_dependencies();
        let ctx = InstantiateCtx::from((
            deps.as_mut(),
            mock_env(),
            message_info(&sender, &[Coin::new(1000u32, "stake")]),
        ));

        let hashlock = {
            let mut hasher = Keccak256::new();
            hasher.update(b"secret");
            hex::encode(&hasher.finalize())
        };

        let order_hash = {
            let mut hasher = Keccak256::new();
            hasher.update(b"orderhash");
            hex::encode(&hasher.finalize()) //.to_ascii_lowercase()
        };

        let insta_data = InstantiateMsgData {
            rescue_delay: 1,
            hashlock,
            order_hash,
            maker: Addr::unchecked("maker"),
            taker: Addr::unchecked("taker"),
            timelocks: Timelocks {
                withdrawal: 1000,
                public_withdrawal: 2000,
                dest_cancellation: 1753880620,
                src_cancellation: 3000,
                src_withdrawal: 5000,
            },
            token: Coin::new(1000u32, "stake"),
        };
        contract.instantiate(ctx, insta_data).unwrap();
        let query_ctx = QueryCtx::from((deps.as_ref(), mock_env()));
        let res = contract.get_timelocks(query_ctx).unwrap();

        println!("timeloks {}", res.timelocks.dest_cancellation);
        assert_eq!(1753880620, res.timelocks.dest_cancellation);
    }

    #[test]
    fn should_public_withdraw() {
        let sender = "alice".into_addr();
        let contract = EscrowDest::<Empty, Empty>::new();
        let mut deps = mock_dependencies();
        let ctx = InstantiateCtx::from((
            deps.as_mut(),
            mock_env(),
            message_info(&sender, &[Coin::new(1000u32, "stake")]),
        ));

        let hashlock = {
            let mut hasher = Keccak256::new();
            hasher.update(b"secret");
            hex::encode(&hasher.finalize())
        };

        let order_hash = {
            let mut hasher = Keccak256::new();
            hasher.update(b"orderhash");
            hex::encode(&hasher.finalize()) //.to_ascii_lowercase()
        };

        let insta_data = InstantiateMsgData {
            rescue_delay: 1,
            hashlock,
            order_hash,
            maker: Addr::unchecked("maker"),
            taker: Addr::unchecked("taker"),
            timelocks: Timelocks {
                withdrawal: 1000,
                public_withdrawal: 2000,
                dest_cancellation: 3000,
                src_cancellation: 3000,
                src_withdrawal: 5000,
            },
            token: Coin::new(1000u32, "stake"),
        };

        contract.instantiate(ctx, insta_data).unwrap();

        let mut mock_env2 = mock_env();
        mock_env2.block.time = Timestamp::from_seconds(2500);

        let taker = Addr::unchecked("bob");
        let exe_ctx = ExecCtx::from((deps.as_mut(), mock_env2, message_info(&taker, &[])));

        contract.public_withdraw(
                exe_ctx,
                WithdrawMsg {
                    secret: String::from("secret"),
                },
            )
            .unwrap();

    }

    #[test]
    fn should_cancel() {
        let sender = "alice".into_addr();
        let contract = EscrowDest::<Empty, Empty>::new();
        let mut deps = mock_dependencies();
        let ctx = InstantiateCtx::from((
            deps.as_mut(),
            mock_env(),
            message_info(&sender, &[Coin::new(1000u32, "stake")]),
        ));

        let hashlock = {
            let mut hasher = Keccak256::new();
            hasher.update(b"secret");
            hex::encode(&hasher.finalize())
        };

        let order_hash = {
            let mut hasher = Keccak256::new();
            hasher.update(b"orderhash");
            hex::encode(&hasher.finalize()) //.to_ascii_lowercase()
        };

        let insta_data = InstantiateMsgData {
            rescue_delay: 1,
            hashlock,
            order_hash,
            maker: Addr::unchecked("maker"),
            taker: Addr::unchecked("taker"),
            timelocks: Timelocks {
                withdrawal: 1000,
                public_withdrawal: 2000,
                dest_cancellation: 3000,
                src_cancellation: 3000,
                src_withdrawal: 5000,
            },
            token: Coin::new(1000u32, "stake"),
        };

        contract.instantiate(ctx, insta_data).unwrap();

        let mut mock_env2 = mock_env();
        mock_env2.block.time = Timestamp::from_seconds(3500);

        let taker = Addr::unchecked("taker");
        let exe_ctx = ExecCtx::from((deps.as_mut(), mock_env2, message_info(&taker, &[])));

        contract.cancel(
                exe_ctx,
            )
            .unwrap();



    }

    #[test]
    fn should_rescue_funds() {
        let sender = "alice".into_addr();
        let contract = EscrowDest::<Empty, Empty>::new();
        let mut deps = mock_dependencies();
        let ctx = InstantiateCtx::from((
            deps.as_mut(),
            mock_env(),
            message_info(&sender, &[Coin::new(1000u32, "stake")]),
        ));

        let hashlock = {
            let mut hasher = Keccak256::new();
            hasher.update(b"secret");
            hex::encode(&hasher.finalize())
        };

        let order_hash = {
            let mut hasher = Keccak256::new();
            hasher.update(b"orderhash");
            hex::encode(&hasher.finalize()) //.to_ascii_lowercase()
        };

        let insta_data = InstantiateMsgData {
            rescue_delay: 1,
            hashlock,
            order_hash,
            maker: Addr::unchecked("maker"),
            taker: Addr::unchecked("taker"),
            timelocks: Timelocks {
                withdrawal: 1000,
                public_withdrawal: 2000,
                dest_cancellation: 3000,
                src_cancellation: 3000,
                src_withdrawal: 5000,
            },
            token: Coin::new(1000u32, "stake"),
        };

        contract.instantiate(ctx, insta_data).unwrap();

        let mut mock_env2 = mock_env();
        mock_env2.block.time = Timestamp::from_seconds(5010);

        let taker = Addr::unchecked("taker");
        let exe_ctx = ExecCtx::from((deps.as_mut(), mock_env2, message_info(&taker, &[])));

        contract.rescue_funds(
                exe_ctx,
            )
            .unwrap();



    }

}
