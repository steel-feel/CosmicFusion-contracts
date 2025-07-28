use cw_storage_plus::Item;
use sylvia::{contract};
// use sha3::{Keccak256};
use sylvia::ctx::{ExecCtx, InstantiateCtx, QueryCtx};
use sylvia::cw_schema::cw_serde;
#[cfg(not(feature = "library"))]
use sylvia::cw_std::Empty;
use sylvia::cw_std::{Response, StdResult, Uint256};
use sylvia::types::{CustomMsg, CustomQuery};
use crate::states::Immutables;

pub struct EscrowDest<E, Q> {
    pub rescue_delay: Item<Uint256>,
    pub immutables: Item<Immutables>,
    _phantom: std::marker::PhantomData<(E, Q)>,
}

#[cw_serde(crate = "sylvia::cw_schema")]
pub struct InstantiateMsgData {
    pub rescue_delay: Uint256,
    pub dst_immutables : Immutables,
}

#[cfg_attr(not(feature = "library"), sylvia::entry_points(generics<Empty, Empty>))]
#[contract]
#[sv::custom(msg = E, query = Q)]
impl<E, Q> EscrowDest<E, Q>
where
    E: CustomMsg + 'static,
    Q: CustomQuery + 'static,
{   //TODO: check if can pass anything in args
    pub const fn new() -> Self {
        Self {
            rescue_delay: Item::new("rescue_delay"),
            immutables: Item::new("immutables"),
            _phantom: std::marker::PhantomData,
        }
    }

    #[sv::msg(instantiate)]
    fn instantiate(&self, ctx: InstantiateCtx<Q>, data: InstantiateMsgData,) -> StdResult<Response<E>> {
     self.rescue_delay.save(ctx.deps.storage, &data.rescue_delay)?;
     self.immutables.save(ctx.deps.storage, &data.dst_immutables)?;
        Ok(Response::new())
    }

    // #[sv::msg(exec)]
    // fn increment(&self, ctx: ExecCtx<Q>) -> StdResult<Response<E>> {
    //     self.count
    //         .update(ctx.deps.storage, |count| -> StdResult<u64> {
    //             Ok(count + 1)
    //         })?;
    //     Ok(Response::new())
    // }

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
        let ctx = InstantiateCtx::from((deps.as_mut(), mock_env(), message_info(&sender, &[])));
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
            rescue_delay : Uint256::from(1 as u32),
          dst_immutables : Immutables {
                     hashlock,
                     order_hash,
                     maker : Addr::unchecked("maker"),
                     taker : Addr::unchecked("taker"),
                     timelocks : Timelocks {
                        withdrawal: Uint256::from(1 as u32),
                        public_withdrawal: Uint256::from(2u32),
                        dest_cancellation: Uint256::from(3u32),
                        src_cancellation : Uint256::from(4u32),
                    }    ,
                    token : Coin::new(1000u32, "stake")   
            }
        };

      
        contract.instantiate(ctx, insta_data).unwrap();

        // We're inspecting the raw storage here, which is fine in unit tests. In
        // integration tests, you should not inspect the internal state like this,
        // but observe the external results.
        // assert_eq!(0, contract..load(deps.as_ref().storage).unwrap());
        assert_eq!(Uint256::one(), contract.rescue_delay.load(deps.as_ref().storage).unwrap());
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
