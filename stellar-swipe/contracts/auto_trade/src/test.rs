#![cfg(test)]

use super::*;
use crate::risk;
use crate::storage;
use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger as _},
    Env,
};

fn setup_env() -> Env {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1000);
    env
}

fn setup_signal(_env: &Env, signal_id: u64, expiry: u64) -> storage::Signal {
    storage::Signal {
        signal_id,
        price: 100,
        expiry,
        base_asset: 1,
    }
}

#[test]
fn test_execute_trade_invalid_amount() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);

    env.as_contract(&contract_id, || {
        let res =
            AutoTradeContract::execute_trade(env.clone(), user.clone(), 1, OrderType::Market, 0);

        assert_eq!(res, Err(AutoTradeError::InvalidAmount));
    });
}

#[test]
fn test_execute_trade_signal_not_found() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);

    env.as_contract(&contract_id, || {
        let res = AutoTradeContract::execute_trade(
            env.clone(),
            user.clone(),
            999,
            OrderType::Market,
            100,
        );

        assert_eq!(res, Err(AutoTradeError::SignalNotFound));
    });
}

#[test]
fn test_execute_trade_signal_expired() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 1;
    let signal = setup_signal(&env, signal_id, env.ledger().timestamp() - 1);

    env.as_contract(&contract_id, || {
        storage::set_signal(&env, signal_id, &signal);
        let res = AutoTradeContract::execute_trade(
            env.clone(),
            user.clone(),
            signal_id,
            OrderType::Market,
            100,
        );

        assert_eq!(res, Err(AutoTradeError::SignalExpired));
    });
}

#[test]
fn test_execute_trade_unauthorized() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 1;
    let signal = setup_signal(&env, signal_id, env.ledger().timestamp() + 1000);

    env.as_contract(&contract_id, || {
        storage::set_signal(&env, signal_id, &signal);
        let res = AutoTradeContract::execute_trade(
            env.clone(),
            user.clone(),
            signal_id,
            OrderType::Market,
            100,
        );

        assert_eq!(res, Err(AutoTradeError::Unauthorized));
    });
}

#[test]
fn test_execute_trade_insufficient_balance() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 1;
    let signal = setup_signal(&env, signal_id, env.ledger().timestamp() + 1000);

    env.as_contract(&contract_id, || {
        storage::set_signal(&env, signal_id, &signal);
        storage::authorize_user(&env, &user);
        env.storage()
            .temporary()
            .set(&(user.clone(), symbol_short!("balance")), &50i128);

        let res = AutoTradeContract::execute_trade(
            env.clone(),
            user.clone(),
            signal_id,
            OrderType::Market,
            100,
        );

        assert_eq!(res, Err(AutoTradeError::InsufficientBalance));
    });
}

#[test]
fn test_execute_trade_market_full_fill() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 1;
    let signal = setup_signal(&env, signal_id, env.ledger().timestamp() + 1000);

    env.as_contract(&contract_id, || {
        storage::set_signal(&env, signal_id, &signal);
        storage::authorize_user(&env, &user);
        env.storage()
            .temporary()
            .set(&(user.clone(), symbol_short!("balance")), &500i128);
        env.storage()
            .temporary()
            .set(&(symbol_short!("liquidity"), signal_id), &500i128);

        let res = AutoTradeContract::execute_trade(
            env.clone(),
            user.clone(),
            signal_id,
            OrderType::Market,
            400,
        )
        .unwrap();

        assert_eq!(res.trade.executed_amount, 400);
        assert_eq!(res.trade.executed_price, 100);
        assert_eq!(res.trade.status, TradeStatus::Filled);
    });
}

#[test]
fn test_execute_trade_market_partial_fill() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 2;
    let signal = setup_signal(&env, signal_id, env.ledger().timestamp() + 1000);

    env.as_contract(&contract_id, || {
        storage::set_signal(&env, signal_id, &signal);
        storage::authorize_user(&env, &user);
        env.storage()
            .temporary()
            .set(&(user.clone(), symbol_short!("balance")), &500i128);
        env.storage()
            .temporary()
            .set(&(symbol_short!("liquidity"), signal_id), &100i128);

        let res = AutoTradeContract::execute_trade(
            env.clone(),
            user.clone(),
            signal_id,
            OrderType::Market,
            300,
        )
        .unwrap();

        assert_eq!(res.trade.executed_amount, 100);
        assert_eq!(res.trade.executed_price, 100);
        assert_eq!(res.trade.status, TradeStatus::PartiallyFilled);
    });
}

#[test]
fn test_execute_trade_limit_filled() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 3;
    let signal = setup_signal(&env, signal_id, env.ledger().timestamp() + 1000);

    env.as_contract(&contract_id, || {
        storage::set_signal(&env, signal_id, &signal);
        storage::authorize_user(&env, &user);
        env.storage()
            .temporary()
            .set(&(user.clone(), symbol_short!("balance")), &500i128);
        env.storage()
            .temporary()
            .set(&(symbol_short!("price"), signal_id), &90i128);

        let res = AutoTradeContract::execute_trade(
            env.clone(),
            user.clone(),
            signal_id,
            OrderType::Limit,
            200,
        )
        .unwrap();

        assert_eq!(res.trade.executed_amount, 200);
        assert_eq!(res.trade.executed_price, 100);
        assert_eq!(res.trade.status, TradeStatus::Filled);
    });
}

#[test]
fn test_execute_trade_limit_not_filled() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 4;
    let signal = setup_signal(&env, signal_id, env.ledger().timestamp() + 1000);

    env.as_contract(&contract_id, || {
        storage::set_signal(&env, signal_id, &signal);
        storage::authorize_user(&env, &user);
        env.storage()
            .temporary()
            .set(&(user.clone(), symbol_short!("balance")), &500i128);
        env.storage()
            .temporary()
            .set(&(symbol_short!("price"), signal_id), &150i128);

        let res = AutoTradeContract::execute_trade(
            env.clone(),
            user.clone(),
            signal_id,
            OrderType::Limit,
            200,
        )
        .unwrap();

        assert_eq!(res.trade.executed_amount, 0);
        assert_eq!(res.trade.executed_price, 0);
        assert_eq!(res.trade.status, TradeStatus::Failed);
    });
}

#[test]
fn test_get_trade_existing() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 1;
    let signal = setup_signal(&env, signal_id, env.ledger().timestamp() + 1000);

    env.as_contract(&contract_id, || {
        storage::set_signal(&env, signal_id, &signal);
        storage::authorize_user(&env, &user);
        env.storage()
            .temporary()
            .set(&(user.clone(), symbol_short!("balance")), &500i128);
        env.storage()
            .temporary()
            .set(&(symbol_short!("liquidity"), signal_id), &500i128);
    });

    env.as_contract(&contract_id, || {
        let _ = AutoTradeContract::execute_trade(
            env.clone(),
            user.clone(),
            signal_id,
            OrderType::Market,
            400,
        )
        .unwrap();
    });

    env.as_contract(&contract_id, || {
        let trade = AutoTradeContract::get_trade(env.clone(), user.clone(), signal_id).unwrap();

        assert_eq!(trade.executed_amount, 400);
    });
}

#[test]
fn test_get_trade_non_existing() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 999;

    env.as_contract(&contract_id, || {
        let trade = AutoTradeContract::get_trade(env.clone(), user.clone(), signal_id);

        assert!(trade.is_none());
    });
}

// ========================================
// Risk Management Tests
// ========================================

#[test]
fn test_get_default_risk_config() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);

    env.as_contract(&contract_id, || {
        let config = AutoTradeContract::get_risk_config(env.clone(), user.clone());

        assert_eq!(config.max_position_pct, 20);
        assert_eq!(config.daily_trade_limit, 10);
        assert_eq!(config.stop_loss_pct, 15);
    });
}

#[test]
fn test_set_custom_risk_config() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);

    env.as_contract(&contract_id, || {
        let custom_config = risk::RiskConfig {
            max_position_pct: 30,
            daily_trade_limit: 15,
            stop_loss_pct: 10,
        };

        AutoTradeContract::set_risk_config(env.clone(), user.clone(), custom_config.clone());

        let retrieved = AutoTradeContract::get_risk_config(env.clone(), user.clone());
        assert_eq!(retrieved, custom_config);
    });
}

#[test]
fn test_position_limit_allows_first_trade() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 1;
    let signal = setup_signal(&env, signal_id, env.ledger().timestamp() + 1000);

    env.as_contract(&contract_id, || {
        storage::set_signal(&env, signal_id, &signal);
        storage::authorize_user(&env, &user);
        env.storage()
            .temporary()
            .set(&(user.clone(), symbol_short!("balance")), &1000i128);
        env.storage()
            .temporary()
            .set(&(symbol_short!("liquidity"), signal_id), &1000i128);

        // First trade should be allowed
        let res = AutoTradeContract::execute_trade(
            env.clone(),
            user.clone(),
            signal_id,
            OrderType::Market,
            1000,
        );

        assert!(res.is_ok());
    });
}

#[test]
fn test_get_user_positions() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 1;
    let signal = setup_signal(&env, signal_id, env.ledger().timestamp() + 1000);

    env.as_contract(&contract_id, || {
        storage::set_signal(&env, signal_id, &signal);
        storage::authorize_user(&env, &user);
        env.storage()
            .temporary()
            .set(&(user.clone(), symbol_short!("balance")), &1000i128);
        env.storage()
            .temporary()
            .set(&(symbol_short!("liquidity"), signal_id), &500i128);

        // Execute a trade
        let _ = AutoTradeContract::execute_trade(
            env.clone(),
            user.clone(),
            signal_id,
            OrderType::Market,
            400,
        )
        .unwrap();

        // Check positions
        let positions = AutoTradeContract::get_user_positions(env.clone(), user.clone());
        assert!(positions.contains_key(1));

        let position = positions.get(1).unwrap();
        assert_eq!(position.amount, 400);
        assert_eq!(position.entry_price, 100);
    });
}

#[test]
fn test_stop_loss_check() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);

    env.as_contract(&contract_id, || {
        // Setup a position with entry price 100
        risk::update_position(&env, &user, 1, 1000, 100);

        let config = risk::RiskConfig::default(); // 15% stop loss

        // Price at 90 (10% drop) - should NOT trigger
        let triggered = risk::check_stop_loss(&env, &user, 1, 90, &config);
        assert!(!triggered);

        // Price at 80 (20% drop) - should trigger
        let triggered = risk::check_stop_loss(&env, &user, 1, 80, &config);
        assert!(triggered);
    });
}

#[test]
fn test_get_trade_history_paginated() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 1;
    let signal = setup_signal(&env, signal_id, env.ledger().timestamp() + 1000);

    // Setup (max_position_pct: 100 so multiple buys in same asset pass risk checks)
    env.as_contract(&contract_id, || {
        storage::set_signal(&env, signal_id, &signal);
        storage::authorize_user(&env, &user);
        risk::set_risk_config(
            &env,
            &user,
            &risk::RiskConfig {
                max_position_pct: 100,
                daily_trade_limit: 10,
                stop_loss_pct: 15,
            },
        );
        env.storage()
            .temporary()
            .set(&(user.clone(), symbol_short!("balance")), &5000i128);
        env.storage()
            .temporary()
            .set(&(symbol_short!("liquidity"), signal_id), &5000i128);
    });

    // Execute 5 trades in separate frames (avoids "frame is already authorized")
    for _ in 0..5 {
        env.as_contract(&contract_id, || {
            let _ = AutoTradeContract::execute_trade(
                env.clone(),
                user.clone(),
                signal_id,
                OrderType::Market,
                100,
            )
            .unwrap();
        });
    }

    // Query history (no auth required)
    env.as_contract(&contract_id, || {
        let history = AutoTradeContract::get_trade_history(env.clone(), user.clone(), 0, 10);
        assert_eq!(history.len(), 5);

        let page2 = AutoTradeContract::get_trade_history(env.clone(), user.clone(), 2, 2);
        assert_eq!(page2.len(), 2);
    });
}

#[test]
fn test_get_trade_history_empty() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);

    env.as_contract(&contract_id, || {
        let history = AutoTradeContract::get_trade_history(env.clone(), user.clone(), 0, 20);
        assert_eq!(history.len(), 0);
    });
}

#[test]
fn test_get_portfolio() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 1;
    let signal = setup_signal(&env, signal_id, env.ledger().timestamp() + 1000);

    env.as_contract(&contract_id, || {
        storage::set_signal(&env, signal_id, &signal);
        storage::authorize_user(&env, &user);
        env.storage()
            .temporary()
            .set(&(user.clone(), symbol_short!("balance")), &1000i128);
        env.storage()
            .temporary()
            .set(&(symbol_short!("liquidity"), signal_id), &500i128);

        let _ = AutoTradeContract::execute_trade(
            env.clone(),
            user.clone(),
            signal_id,
            OrderType::Market,
            400,
        )
        .unwrap();

        let portfolio = AutoTradeContract::get_portfolio(env.clone(), user.clone());
        assert_eq!(portfolio.assets.len(), 1);
        assert_eq!(portfolio.assets.get(0).unwrap().amount, 400);
        assert_eq!(portfolio.assets.get(0).unwrap().asset_id, 1);
    });
}

#[test]
fn test_portfolio_value_calculation() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);

    env.as_contract(&contract_id, || {
        // Set up positions and prices
        risk::set_asset_price(&env, 1, 100);
        risk::set_asset_price(&env, 2, 200);

        risk::update_position(&env, &user, 1, 1000, 100);
        risk::update_position(&env, &user, 2, 500, 200);

        let total_value = risk::calculate_portfolio_value(&env, &user);
        // (1000 * 100 / 100) + (500 * 200 / 100) = 1000 + 1000 = 2000
        assert_eq!(total_value, 2000);
    });
}

// ========================================
// Authorization Tests
// ========================================

#[test]
fn test_grant_authorization_success() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);

    env.as_contract(&contract_id, || {
        let res = AutoTradeContract::grant_authorization(env.clone(), user.clone(), 500_0000000, 30);
        assert!(res.is_ok());

        let config = AutoTradeContract::get_auth_config(env.clone(), user.clone()).unwrap();
        assert_eq!(config.authorized, true);
        assert_eq!(config.max_trade_amount, 500_0000000);
        assert_eq!(config.expires_at, 1000 + (30 * 86400));
    });
}

#[test]
fn test_grant_authorization_zero_amount() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);

    env.as_contract(&contract_id, || {
        let res = AutoTradeContract::grant_authorization(env.clone(), user.clone(), 0, 30);
        assert_eq!(res, Err(AutoTradeError::InvalidAmount));
    });
}

#[test]
fn test_revoke_authorization() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);

    env.as_contract(&contract_id, || {
        AutoTradeContract::grant_authorization(env.clone(), user.clone(), 1000_0000000, 30)
            .unwrap();
        AutoTradeContract::revoke_authorization(env.clone(), user.clone()).unwrap();

        let config = AutoTradeContract::get_auth_config(env.clone(), user.clone());
        assert!(config.is_none());
    });
}

#[test]
fn test_trade_under_limit_succeeds() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 1;
    let signal = setup_signal(&env, signal_id, env.ledger().timestamp() + 1000);

    env.as_contract(&contract_id, || {
        storage::set_signal(&env, signal_id, &signal);
        AutoTradeContract::grant_authorization(env.clone(), user.clone(), 500_0000000, 30)
            .unwrap();
        env.storage()
            .temporary()
            .set(&(user.clone(), symbol_short!("balance")), &1000_0000000i128);
        env.storage()
            .temporary()
            .set(&(symbol_short!("liquidity"), signal_id), &1000_0000000i128);

        let res = AutoTradeContract::execute_trade(
            env.clone(),
            user.clone(),
            signal_id,
            OrderType::Market,
            400_0000000,
        );
        assert!(res.is_ok());
    });
}

#[test]
fn test_trade_over_limit_fails() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 1;
    let signal = setup_signal(&env, signal_id, env.ledger().timestamp() + 1000);

    env.as_contract(&contract_id, || {
        storage::set_signal(&env, signal_id, &signal);
        AutoTradeContract::grant_authorization(env.clone(), user.clone(), 500_0000000, 30)
            .unwrap();
        env.storage()
            .temporary()
            .set(&(user.clone(), symbol_short!("balance")), &1000_0000000i128);

        let res = AutoTradeContract::execute_trade(
            env.clone(),
            user.clone(),
            signal_id,
            OrderType::Market,
            600_0000000,
        );
        assert_eq!(res, Err(AutoTradeError::Unauthorized));
    });
}

#[test]
fn test_revoked_authorization_blocks_trade() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 1;
    let signal = setup_signal(&env, signal_id, env.ledger().timestamp() + 1000);

    env.as_contract(&contract_id, || {
        storage::set_signal(&env, signal_id, &signal);
        AutoTradeContract::grant_authorization(env.clone(), user.clone(), 1000_0000000, 30)
            .unwrap();
        AutoTradeContract::revoke_authorization(env.clone(), user.clone()).unwrap();

        let res = AutoTradeContract::execute_trade(
            env.clone(),
            user.clone(),
            signal_id,
            OrderType::Market,
            100_0000000,
        );
        assert_eq!(res, Err(AutoTradeError::Unauthorized));
    });
}

#[test]
fn test_expired_authorization_blocks_trade() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 1;
    let signal = setup_signal(&env, signal_id, env.ledger().timestamp() + 100000);

    env.as_contract(&contract_id, || {
        storage::set_signal(&env, signal_id, &signal);
        // Grant with 1 day duration
        AutoTradeContract::grant_authorization(env.clone(), user.clone(), 1000_0000000, 1)
            .unwrap();

        // Fast forward time beyond expiry
        env.ledger().set_timestamp(1000 + 86400 + 1);

        let res = AutoTradeContract::execute_trade(
            env.clone(),
            user.clone(),
            signal_id,
            OrderType::Market,
            100_0000000,
        );
        assert_eq!(res, Err(AutoTradeError::Unauthorized));
    });
}

#[test]
fn test_multiple_authorization_grants_latest_applies() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);

    env.as_contract(&contract_id, || {
        AutoTradeContract::grant_authorization(env.clone(), user.clone(), 500_0000000, 30)
            .unwrap();
        AutoTradeContract::grant_authorization(env.clone(), user.clone(), 1000_0000000, 60)
            .unwrap();

        let config = AutoTradeContract::get_auth_config(env.clone(), user.clone()).unwrap();
        assert_eq!(config.max_trade_amount, 1000_0000000);
        assert_eq!(config.expires_at, 1000 + (60 * 86400));
    });
}

#[test]
fn test_authorization_at_exact_limit() {
    let env = setup_env();
    let contract_id = env.register(AutoTradeContract, ());
    let user = Address::generate(&env);
    let signal_id = 1;
    let signal = setup_signal(&env, signal_id, env.ledger().timestamp() + 1000);

    env.as_contract(&contract_id, || {
        storage::set_signal(&env, signal_id, &signal);
        AutoTradeContract::grant_authorization(env.clone(), user.clone(), 500_0000000, 30)
            .unwrap();
        env.storage()
            .temporary()
            .set(&(user.clone(), symbol_short!("balance")), &1000_0000000i128);
        env.storage()
            .temporary()
            .set(&(symbol_short!("liquidity"), signal_id), &1000_0000000i128);

        let res = AutoTradeContract::execute_trade(
            env.clone(),
            user.clone(),
            signal_id,
            OrderType::Market,
            500_0000000,
        );
        assert!(res.is_ok());
    });
}

// ========================================
// Referral & Reward System Tests
// ========================================

#[cfg(test)]
mod referral_tests {
    use super::*;
    use crate::referral::{self, REFERRAL_WINDOW_SECS, MAX_REFERRAL_TRADES, MAX_ACTIVE_REFERRALS};
    use soroban_sdk::{
        testutils::{Address as _, Ledger as _},
        Env,
    };

    fn setup_env() -> Env {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set_timestamp(1_000_000);
        env
    }

    fn setup_signal_and_user(env: &Env, contract_id: &Address) -> (Address, u64) {
        let user = Address::generate(env);
        let signal_id = 42u64;
        env.as_contract(contract_id, || {
            storage::set_signal(
                env,
                signal_id,
                &storage::Signal {
                    signal_id,
                    price: 100,
                    expiry: env.ledger().timestamp() + 1_000_000,
                    base_asset: 1,
                },
            );
            storage::authorize_user(env, &user);
            env.storage()
                .temporary()
                .set(&(symbol_short!("liquidity"), signal_id), &1_000_000_000i128);
        });
        (user, signal_id)
    }

    // ── set_referrer via contract ─────────────────────────────────────────────

    #[test]
    fn test_contract_set_referrer_success() {
        let env = setup_env();
        let contract_id = env.register(AutoTradeContract, ());
        let referrer = Address::generate(&env);
        let referee = Address::generate(&env);

        env.as_contract(&contract_id, || {
            AutoTradeContract::set_referrer(env.clone(), referee.clone(), referrer.clone())
                .unwrap();

            let entry =
                AutoTradeContract::get_referral_entry(env.clone(), referee.clone()).unwrap();
            assert_eq!(entry.referrer, referrer);
            assert_eq!(entry.trade_count, 0);
        });
    }

    #[test]
    fn test_contract_self_referral_blocked() {
        let env = setup_env();
        let contract_id = env.register(AutoTradeContract, ());
        let user = Address::generate(&env);

        env.as_contract(&contract_id, || {
            let err =
                AutoTradeContract::set_referrer(env.clone(), user.clone(), user.clone())
                    .unwrap_err();
            assert_eq!(err, AutoTradeError::SelfReferral);
        });
    }

    #[test]
    fn test_contract_referral_already_set_blocked() {
        let env = setup_env();
        let contract_id = env.register(AutoTradeContract, ());
        let referrer = Address::generate(&env);
        let referee = Address::generate(&env);

        env.as_contract(&contract_id, || {
            AutoTradeContract::set_referrer(env.clone(), referee.clone(), referrer.clone())
                .unwrap();
            let err =
                AutoTradeContract::set_referrer(env.clone(), referee.clone(), referrer.clone())
                    .unwrap_err();
            assert_eq!(err, AutoTradeError::ReferralAlreadySet);
        });
    }

    #[test]
    fn test_contract_circular_referral_blocked() {
        let env = setup_env();
        let contract_id = env.register(AutoTradeContract, ());
        let a = Address::generate(&env);
        let b = Address::generate(&env);
        let c = Address::generate(&env);

        env.as_contract(&contract_id, || {
            // A → B → C, then C tries to refer A (circular)
            AutoTradeContract::set_referrer(env.clone(), b.clone(), a.clone()).unwrap();
            AutoTradeContract::set_referrer(env.clone(), c.clone(), b.clone()).unwrap();
            let err =
                AutoTradeContract::set_referrer(env.clone(), a.clone(), c.clone()).unwrap_err();
            assert_eq!(err, AutoTradeError::CircularReferral);
        });
    }

    #[test]
    fn test_contract_max_referral_limit_101st_blocked() {
        let env = setup_env();
        let contract_id = env.register(AutoTradeContract, ());
        let referrer = Address::generate(&env);

        env.as_contract(&contract_id, || {
            // Manually set active_referrals to MAX
            use crate::referral::{ReferralStats, ReferralKey};
            use soroban_sdk::Map;
            let stats = ReferralStats {
                total_referrals: MAX_ACTIVE_REFERRALS,
                active_referrals: MAX_ACTIVE_REFERRALS,
                total_earnings: 0,
                earnings_by_asset: Map::new(&env),
            };
            env.storage()
                .persistent()
                .set(&ReferralKey::Stats(referrer.clone()), &stats);

            let new_referee = Address::generate(&env);
            let err =
                AutoTradeContract::set_referrer(env.clone(), new_referee, referrer.clone())
                    .unwrap_err();
            assert_eq!(err, AutoTradeError::ReferralLimitExceeded);
        });
    }

    // ── Reward earned on trade execution ─────────────────────────────────────

    #[test]
    fn test_referrer_earns_10_percent_of_platform_fee_on_trade() {
        let env = setup_env();
        let contract_id = env.register(AutoTradeContract, ());
        let referrer = Address::generate(&env);
        let (referee, signal_id) = setup_signal_and_user(&env, &contract_id);

        env.as_contract(&contract_id, || {
            AutoTradeContract::set_referrer(env.clone(), referee.clone(), referrer.clone())
                .unwrap();

            AutoTradeContract::execute_trade(
                env.clone(),
                referee.clone(),
                signal_id,
                OrderType::Market,
                10_000_000,
            )
            .unwrap();

            let stats =
                AutoTradeContract::get_referral_stats(env.clone(), referrer.clone());
            // platform_fee = 10_000_000 * 7 / 100 = 700_000
            // referral_reward = 700_000 * 10 / 100 = 70_000
            assert_eq!(stats.total_earnings, 70_000);
        });
    }

    #[test]
    fn test_referrer_earns_across_10_trades() {
        let env = setup_env();
        let contract_id = env.register(AutoTradeContract, ());
        let referrer = Address::generate(&env);
        let (referee, signal_id) = setup_signal_and_user(&env, &contract_id);

        env.as_contract(&contract_id, || {
            AutoTradeContract::set_referrer(env.clone(), referee.clone(), referrer.clone())
                .unwrap();
        });

        for _ in 0..10 {
            env.as_contract(&contract_id, || {
                AutoTradeContract::execute_trade(
                    env.clone(),
                    referee.clone(),
                    signal_id,
                    OrderType::Market,
                    10_000_000,
                )
                .unwrap();
            });
        }

        env.as_contract(&contract_id, || {
            let stats = AutoTradeContract::get_referral_stats(env.clone(), referrer.clone());
            assert_eq!(stats.total_earnings, 10 * 70_000);
        });
    }

    #[test]
    fn test_reward_stops_after_90_days_simulation() {
        let env = setup_env();
        let contract_id = env.register(AutoTradeContract, ());
        let referrer = Address::generate(&env);
        let (referee, signal_id) = setup_signal_and_user(&env, &contract_id);

        env.as_contract(&contract_id, || {
            AutoTradeContract::set_referrer(env.clone(), referee.clone(), referrer.clone())
                .unwrap();

            // Earn some rewards first
            AutoTradeContract::execute_trade(
                env.clone(),
                referee.clone(),
                signal_id,
                OrderType::Market,
                10_000_000,
            )
            .unwrap();

            let earnings_before =
                AutoTradeContract::get_referral_stats(env.clone(), referrer.clone())
                    .total_earnings;
            assert!(earnings_before > 0);

            // Advance past 90 days
            env.ledger()
                .set_timestamp(1_000_000 + REFERRAL_WINDOW_SECS + 1);

            // Update signal expiry so trade itself doesn't fail
            storage::set_signal(
                &env,
                signal_id,
                &storage::Signal {
                    signal_id,
                    price: 100,
                    expiry: env.ledger().timestamp() + 1_000_000,
                    base_asset: 1,
                },
            );

            AutoTradeContract::execute_trade(
                env.clone(),
                referee.clone(),
                signal_id,
                OrderType::Market,
                10_000_000,
            )
            .unwrap();

            // Earnings must not have increased
            let earnings_after =
                AutoTradeContract::get_referral_stats(env.clone(), referrer.clone())
                    .total_earnings;
            assert_eq!(earnings_before, earnings_after);
        });
    }

    #[test]
    fn test_reward_stops_after_100_trades() {
        let env = setup_env();
        let contract_id = env.register(AutoTradeContract, ());
        let referrer = Address::generate(&env);
        let (referee, signal_id) = setup_signal_and_user(&env, &contract_id);

        env.as_contract(&contract_id, || {
            AutoTradeContract::set_referrer(env.clone(), referee.clone(), referrer.clone())
                .unwrap();
        });

        // Execute exactly MAX_REFERRAL_TRADES trades
        for _ in 0..MAX_REFERRAL_TRADES {
            env.as_contract(&contract_id, || {
                AutoTradeContract::execute_trade(
                    env.clone(),
                    referee.clone(),
                    signal_id,
                    OrderType::Market,
                    10_000_000,
                )
                .unwrap();
            });
        }

        let earnings_at_cap = env.as_contract(&contract_id, || {
            AutoTradeContract::get_referral_stats(env.clone(), referrer.clone()).total_earnings
        });

        // 101st trade — no more reward
        env.as_contract(&contract_id, || {
            AutoTradeContract::execute_trade(
                env.clone(),
                referee.clone(),
                signal_id,
                OrderType::Market,
                10_000_000,
            )
            .unwrap();
        });

        env.as_contract(&contract_id, || {
            let earnings_after =
                AutoTradeContract::get_referral_stats(env.clone(), referrer.clone())
                    .total_earnings;
            assert_eq!(earnings_at_cap, earnings_after);
        });
    }

    #[test]
    fn test_no_reward_without_referral_on_trade() {
        let env = setup_env();
        let contract_id = env.register(AutoTradeContract, ());
        let (user, signal_id) = setup_signal_and_user(&env, &contract_id);

        env.as_contract(&contract_id, || {
            AutoTradeContract::execute_trade(
                env.clone(),
                user.clone(),
                signal_id,
                OrderType::Market,
                10_000_000,
            )
            .unwrap();

            // No referrer set — stats should be empty
            let stats = AutoTradeContract::get_referral_stats(env.clone(), user.clone());
            assert_eq!(stats.total_earnings, 0);
        });
    }

    // ── Dashboard queries ─────────────────────────────────────────────────────

    #[test]
    fn test_get_referral_stats_empty() {
        let env = setup_env();
        let contract_id = env.register(AutoTradeContract, ());
        let user = Address::generate(&env);

        env.as_contract(&contract_id, || {
            let stats = AutoTradeContract::get_referral_stats(env.clone(), user.clone());
            assert_eq!(stats.total_referrals, 0);
            assert_eq!(stats.active_referrals, 0);
            assert_eq!(stats.total_earnings, 0);
        });
    }

    #[test]
    fn test_get_referral_entry_none_for_unreferred_user() {
        let env = setup_env();
        let contract_id = env.register(AutoTradeContract, ());
        let user = Address::generate(&env);

        env.as_contract(&contract_id, || {
            let entry = AutoTradeContract::get_referral_entry(env.clone(), user.clone());
            assert!(entry.is_none());
        });
    }
}
