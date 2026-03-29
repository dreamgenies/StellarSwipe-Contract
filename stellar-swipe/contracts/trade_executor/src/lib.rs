#![no_std]

mod wire;

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, vec, Address, Env, IntoVal, Symbol, Vec,
};

/// Maximum copy trades per batch (instruction budget safety).
pub const MAX_BATCH_TRADES: u32 = 10;

/// `max_slippage_bps` must be ≤ 10_000 (100%).
pub const MAX_SLIPPAGE_BPS: u32 = 10_000;

/// Per-result error code when slippage input is invalid (executor layer).
pub const ERR_INVALID_SLIPPAGE_BPS: u32 = 65_534;

/// Per-result error code when the auto_trade invoke aborts (host / non-contract error).
pub const ERR_INVOKE_ABORT: u32 = 65_535;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ContractError {
    NotInitialized = 1,
    BatchSizeExceeded = 2,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CopyTradeRequest {
    pub signal_id: u64,
    pub amount: i128,
    pub max_slippage_bps: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CopyTradeResult {
    pub signal_id: u64,
    pub success: bool,
    pub error: Option<u32>,
}

#[contracttype]
pub enum StorageKey {
    AutoTrade,
}

#[contract]
pub struct TradeExecutorContract;

#[contractimpl]
impl TradeExecutorContract {
    /// Store the deployed auto_trade contract address.
    pub fn initialize(env: Env, auto_trade: Address) {
        env.storage()
            .persistent()
            .set(&StorageKey::AutoTrade, &auto_trade);
    }

    /// Execute up to [`MAX_BATCH_TRADES`] copy trades in one transaction.
    ///
    /// Each entry in `trades` is processed independently; one failure does not roll back others.
    /// Emits `BatchExecuted` with the full result vector.
    pub fn batch_execute(
        env: Env,
        user: Address,
        trades: Vec<CopyTradeRequest>,
    ) -> Result<Vec<CopyTradeResult>, ContractError> {
        if trades.len() > MAX_BATCH_TRADES as u32 {
            return Err(ContractError::BatchSizeExceeded);
        }

        user.require_auth();

        let auto_trade: Address = env
            .storage()
            .persistent()
            .get(&StorageKey::AutoTrade)
            .ok_or(ContractError::NotInitialized)?;

        let mut results: Vec<CopyTradeResult> = Vec::new(&env);
        let exec_sym = Symbol::new(&env, "execute_trade");

        for i in 0..trades.len() {
            let req = trades.get(i).unwrap();

            if req.max_slippage_bps > MAX_SLIPPAGE_BPS {
                results.push_back(CopyTradeResult {
                    signal_id: req.signal_id,
                    success: false,
                    error: Some(ERR_INVALID_SLIPPAGE_BPS),
                });
                continue;
            }

            let _ = req.max_slippage_bps; // reserved for future slippage checks in auto_trade

            let args = vec![
                &env,
                user.to_val(),
                req.signal_id.into_val(&env),
                wire::OrderType::Market.into_val(&env),
                req.amount.into_val(&env),
            ];

            match env.try_invoke_contract::<wire::TradeResult, wire::AutoTradeError>(
                &auto_trade,
                &exec_sym,
                args,
            ) {
                Ok(Ok(_trade_result)) => results.push_back(CopyTradeResult {
                    signal_id: req.signal_id,
                    success: true,
                    error: None,
                }),
                Ok(Err(_)) => results.push_back(CopyTradeResult {
                    signal_id: req.signal_id,
                    success: false,
                    error: Some(ERR_INVOKE_ABORT),
                }),
                Err(Ok(e)) => results.push_back(CopyTradeResult {
                    signal_id: req.signal_id,
                    success: false,
                    error: Some(e as u32),
                }),
                Err(Err(_)) => results.push_back(CopyTradeResult {
                    signal_id: req.signal_id,
                    success: false,
                    error: Some(ERR_INVOKE_ABORT),
                }),
            }
        }

        #[allow(deprecated)]
        env.events().publish(
            (Symbol::new(&env, "BatchExecuted"), user.clone()),
            results.clone(),
        );

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use auto_trade::{set_signal, AutoTradeContract, AutoTradeContractClient, RiskConfig, Signal};
    use soroban_sdk::{symbol_short, testutils::{Address as _, Ledger as _}};

    fn make_signal(_env: &Env, signal_id: u64, expiry: u64) -> Signal {
        Signal {
            signal_id,
            price: 100,
            expiry,
            // Distinct assets so sequential fills do not trip default per-asset position limits.
            base_asset: signal_id as u32,
        }
    }

    #[test]
    fn batch_five_two_fail_three_succeed() {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set_timestamp(1000);

        let admin = Address::generate(&env);
        let user = Address::generate(&env);

        let auto_id = env.register(AutoTradeContract, ());
        let exec_id = env.register(TradeExecutorContract, ());

        let auto_client = AutoTradeContractClient::new(&env, &auto_id);
        auto_client.initialize(&admin);
        auto_client.init_rate_limit_admin(&admin);
        auto_client.try_add_to_whitelist(&user).unwrap();
        auto_client
            .try_grant_authorization(&user, &1_000_000i128, &30u32)
            .unwrap();

        auto_client.set_risk_config(
            &user,
            &RiskConfig {
                max_position_pct: 100,
                daily_trade_limit: 100,
                stop_loss_pct: 100,
                trailing_stop_enabled: false,
                trailing_stop_pct: 1000,
            },
        );

        let expiry = env.ledger().timestamp() + 10_000;
        for sid in [1u64, 2u64, 3u64] {
            env.as_contract(&auto_id, || {
                set_signal(&env, sid, &make_signal(&env, sid, expiry));
            });
        }

        env.as_contract(&auto_id, || {
            env.storage()
                .temporary()
                .set(&(user.clone(), symbol_short!("balance")), &10_000i128);
            for sid in [1u64, 2u64, 3u64] {
                env.storage()
                    .temporary()
                    .set(&(symbol_short!("liquidity"), sid), &500i128);
            }
        });

        let exec_client = TradeExecutorContractClient::new(&env, &exec_id);
        exec_client.initialize(&auto_id);

        let mut trades: Vec<CopyTradeRequest> = Vec::new(&env);
        for sid in [1u64, 2u64, 3u64, 999u64, 1000u64] {
            trades.push_back(CopyTradeRequest {
                signal_id: sid,
                amount: 40,
                max_slippage_bps: 100,
            });
        }

        let results = exec_client.batch_execute(&user, &trades);
        assert_eq!(results.len(), 5);

        let mut ok = 0u32;
        let mut fail = 0u32;
        for i in 0..results.len() {
            let r = results.get(i).unwrap();
            if r.success {
                ok += 1;
                assert!(r.error.is_none());
            } else {
                fail += 1;
                assert_eq!(r.error, Some(super::wire::AutoTradeError::SignalNotFound as u32));
            }
        }
        assert_eq!(ok, 3);
        assert_eq!(fail, 2);
    }

    #[test]
    fn batch_eleven_returns_batch_size_exceeded() {
        let env = Env::default();
        env.mock_all_auths();

        let auto_id = env.register(AutoTradeContract, ());
        let exec_id = env.register(TradeExecutorContract, ());
        let user = Address::generate(&env);

        AutoTradeContractClient::new(&env, &auto_id).initialize(&Address::generate(&env));
        TradeExecutorContractClient::new(&env, &exec_id).initialize(&auto_id);

        let mut trades: Vec<CopyTradeRequest> = Vec::new(&env);
        for _ in 0..11 {
            trades.push_back(CopyTradeRequest {
                signal_id: 1,
                amount: 1,
                max_slippage_bps: 0,
            });
        }

        let err = TradeExecutorContractClient::new(&env, &exec_id).try_batch_execute(&user, &trades);
        assert_eq!(err, Err(Ok(ContractError::BatchSizeExceeded)));
    }
}
