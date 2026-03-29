#![no_std]

pub mod assets;
pub mod emergency;
 main
pub mod rate_limit;

pub use assets::{validate_asset_pair, Asset, AssetPair, AssetPairError};
pub use emergency::{PauseState, CAT_TRADING, CAT_SIGNALS, CAT_STAKES, CAT_ALL};
pub use rate_limit::{ActionType, RateLimitConfig, check_rate_limit, record_action, set_config as set_rate_limit_config};

pub mod oracle;

pub use assets::{validate_asset_pair, Asset, AssetPair, AssetPairError};
pub use emergency::{PauseState, CAT_TRADING, CAT_SIGNALS, CAT_STAKES, CAT_ALL};
pub use oracle::{IOracleClient, MockOracleClient, OnChainOracleClient, OracleError, OraclePrice};
 main
