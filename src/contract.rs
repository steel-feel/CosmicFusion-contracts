use crate::error::ContractError;
use crate::states::Immutables;
use cw_storage_plus::Item;
use sha3::{Digest, Keccak256};
use sylvia::contract;
use sylvia::ctx::{ExecCtx, InstantiateCtx, QueryCtx};
use sylvia::cw_schema::cw_serde;
#[cfg(not(feature = "library"))]
use sylvia::cw_std::Empty;
use sylvia::cw_std::{Response, StdResult, Timestamp, Uint256};
use sylvia::types::{CustomMsg, CustomQuery};

pub struct EscrowDest<E, Q> {
    pub rescue_delay: Item<Uint256>,
    pub immutables: Item<Immutables>,
    _phantom: std::marker::PhantomData<(E, Q)>,
}

#[cw_serde(crate = "sylvia::cw_schema")]
pub struct InstantiateMsgData {
    pub rescue_delay: Uint256,
    pub dst_immutables: Immutables,
}

#[cw_serde(crate = "sylvia::cw_schema")]
pub struct WithdrawMsg {
    pub secret: String,
}

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
            if asset.denom == data.dst_immutables.token.denom
                && asset.amount == data.dst_immutables.token.amount
            {
                ok = true;
            }
        }
        if !ok {
            return Err(ContractError::UnmatchedDenomOrAmount);
        }

        self.rescue_delay
            .save(ctx.deps.storage, &data.rescue_delay)?;
        self.immutables
            .save(ctx.deps.storage, &data.dst_immutables)?;
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
        // onlyAfter(immutables.timelocks.get(TimelocksLib.Stage.DstWithdrawal))
        if immutables.timelocks.withdrawal > ctx.env.block.time.seconds() {
            return Err(ContractError::DestWithrawTimeLimit);
        }
        // onlyBefore(immutables.timelocks.get(TimelocksLib.Stage.DstCancellation))
        if immutables.timelocks.dest_cancellation < ctx.env.block.time.seconds() {
            return Err(ContractError::DestCancelTimeLimit);
        }
        // Check secret hash
        let mut hasher = Keccak256::new();
        hasher.update(msg.secret.as_bytes());
        let computed_hash = hasher.finalize();

        if computed_hash.to_ascii_lowercase() != immutables.hashlock {
            return Err(ContractError::InvalidSecret);
        }

        Ok(Response::new())
    }

    // #[sv::msg(query)]
    // fn count(&self, ctx: QueryCtx<Q>) -> StdResult<CountResponse> {
    //     let count = self.count.load(ctx.deps.storage)?;
    //     Ok(CountResponse { count })
    // }
}

#[cw_serde(crate = "sylvia")]
pub struct CountResponse {
    pub count: u64,
}

#[cfg(test)]
mod tests {
    use crate::states::Timelocks;

    use super::*;

    use sha3::{Digest, Keccak256};
    use sylvia::cw_multi_test::IntoAddr;
    use sylvia::cw_std::testing::{message_info, mock_dependencies, mock_env};
    use sylvia::cw_std::{Addr, Coin, Empty};

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
        let mut hasher = Keccak256::new();
        hasher.update(b"secret");
        let hashlock = {
            let mut hasher = Keccak256::new();
            hasher.update(b"secret");
            hasher.finalize().to_ascii_lowercase()
        };

        let order_hash = {
            let mut hasher = Keccak256::new();
            hasher.update(b"orderhash");
            hasher.finalize().to_ascii_lowercase()
        };

        let insta_data = InstantiateMsgData {
            rescue_delay: Uint256::from(1 as u32),
            dst_immutables: Immutables {
                hashlock,
                order_hash,
                maker: Addr::unchecked("maker"),
                taker: Addr::unchecked("taker"),
                timelocks: Timelocks {
                    withdrawal: 1,
                    public_withdrawal: 2,
                    dest_cancellation: 3,
                    src_cancellation: 4,
                },
                token: Coin::new(1000u32, "stake"),
            },
        };
        contract.instantiate(ctx, insta_data).unwrap();

        // We're inspecting the raw storage here, which is fine in unit tests. In
        // integration tests, you should not inspect the internal state like this,
        // but observe the external results.
        // assert_eq!(0, contract..load(deps.as_ref().storage).unwrap());
        assert_eq!(
            Uint256::one(),
            contract.rescue_delay.load(deps.as_ref().storage).unwrap()
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
        let mut hasher = Keccak256::new();
        hasher.update(b"secret");
        let hashlock = {
            let mut hasher = Keccak256::new();
            hasher.update(b"secret");
            hasher.finalize().to_ascii_lowercase()
        };

        let order_hash = {
            let mut hasher = Keccak256::new();
            hasher.update(b"orderhash");
            hasher.finalize().to_ascii_lowercase()
        };

        let insta_data = InstantiateMsgData {
            rescue_delay: Uint256::from(1 as u32),
            dst_immutables: Immutables {
                hashlock,
                order_hash,
                maker: Addr::unchecked("maker"),
                taker: Addr::unchecked("taker"),
                timelocks: Timelocks {
                    withdrawal: 1000,
                    public_withdrawal: 2000,
                    dest_cancellation: 3000,
                    src_cancellation: 4000,
                },
                token: Coin::new(1000u32, "stake"),
            },
        };
        contract.instantiate(ctx, insta_data).unwrap();


        let mut mock_env2 = mock_env();
        mock_env2.block.time = Timestamp::from_seconds(1500);

        let taker = Addr::unchecked("taker");
        let exe_ctx = ExecCtx::from((
            deps.as_mut(),
            mock_env2,
            message_info(&taker, &[]),
        ));

        contract.withdraw(exe_ctx, WithdrawMsg { secret: String::from("secret") } ).unwrap();
        
    }

    #[test]
    #[should_panic]
    fn secret_does_not_match() {
        let sender = "alice".into_addr();
        let contract = EscrowDest::<Empty, Empty>::new();
        let mut deps = mock_dependencies();

        let ctx = InstantiateCtx::from((
            deps.as_mut(),
            mock_env(),
            message_info(&sender, &[Coin::new(1000u32, "stake")]),
        ));
        let mut hasher = Keccak256::new();
        hasher.update(b"secret");
        let hashlock = {
            let mut hasher = Keccak256::new();
            hasher.update(b"secret");
            hasher.finalize().to_ascii_lowercase()
        };

        let order_hash = {
            let mut hasher = Keccak256::new();
            hasher.update(b"orderhash");
            hasher.finalize().to_ascii_lowercase()
        };

        let insta_data = InstantiateMsgData {
            rescue_delay: Uint256::from(1 as u32),
            dst_immutables: Immutables {
                hashlock,
                order_hash,
                maker: Addr::unchecked("maker"),
                taker: Addr::unchecked("taker"),
                timelocks: Timelocks {
                    withdrawal: 1000,
                    public_withdrawal: 2000,
                    dest_cancellation: 3000,
                    src_cancellation: 4000,
                },
                token: Coin::new(1000u32, "stake"),
            },
        };
        contract.instantiate(ctx, insta_data).unwrap();

        let taker = Addr::unchecked("taker");
        let exe_ctx = ExecCtx::from((
            deps.as_mut(),
            mock_env(),
            message_info(&taker, &[]),
        ));

      let err =  contract.withdraw(exe_ctx, WithdrawMsg { secret: String::from("secret") } ).unwrap_err();
      assert_eq!(err, ContractError::InvalidSecret);
    }



    // #[test]
    // fn query() {
    //     let sender = "alice".into_addr();
    //     let contract = CounterContract::<Empty, Empty>::new();
    //     let mut deps = mock_dependencies();
    //     let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&sender, &[])));
    //     contract.instantiate(ctx).unwrap();

    //     let ctx = QueryCtx::from((deps.as_ref(), mock_env()));
    //     let res = contract.count(ctx).unwrap();
    //     assert_eq!(0, res.count);
    // }

    // #[test]
    // fn inc() {
    //     let sender = "alice".into_addr();
    //     let contract = CounterContract::<Empty, Empty>::new();
    //     let mut deps = mock_dependencies();
    //     let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&sender, &[])));
    //     contract.instantiate(ctx).unwrap();

    //     let ctx = ExecCtx::from((deps.as_mut(), mock_env(), message_info(&sender, &[])));
    //     contract.increment(ctx).unwrap();

    //     let ctx = QueryCtx::from((deps.as_ref(), mock_env()));
    //     let res = contract.count(ctx).unwrap();
    //     assert_eq!(1, res.count);
    // }
}
