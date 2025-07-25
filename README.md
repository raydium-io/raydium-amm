<div align="center">
  <h1>raydium-amm</h1>
</div>

## Program Deployments

| Environment         |   [PROGRAM](/program)                          |
| ------------------- | ---------------------------------------------- |
| Mainnet             | `675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8` |
| Devnet              | `DRaya7Kj3aMWQSy19kSjvmuwq9docCHofyP9kanQGaav` |

## Overview

- **The Raydium AMM is an on-chain smart contract based on the “constant product” in a permissionless and decentralized manner built on the Solana blockchain.And it also shares its liquidity according to the Fibonacci sequence in the form of limit orders on [OpenBook](https://github.com/openbook-dex/program), the primary central limit order book (CLOB) of Solana**
- **The audit process is [here](https://github.com/raydium-io/raydium-docs/tree/master/audit)**
- **The dev document is [here](https://github.com/raydium-io/raydium-docs/tree/master/dev-resources)**

## Environment Setup
1. Install [Rust](https://www.rust-lang.org/tools/install).
2. Install [Solana](https://docs.solana.com/cli/install-solana-cli-tools) and then run `solana-keygen new` to create a keypair at the default location.

## Build

Clone the repository and enter the source code directory.
```bash
git clone https://github.com/raydium-io/raydium-amm
cd raydium-amm/program
```

### Mainnet Build
```bash
cargo build-sbf
```
### Devnet Build
```bash
cargo build-sbf --features devnet
```
### Localnet Build
You must update these pubkeys in the "config_feature" as yours over the localnet feature before build;

```bash
cargo build-sbf --features localnet
```

After building, the smart contract files are all located in the target directory.

## Deploy
```bash
solana deploy
```
Attention, check your configuration and confirm the environment you want to deploy.

## QuickStart

1. You must have an openbook market not associated to any amm pool if you want to initialize a new amm pool.
  And you can refer to [ListMarket](https://github.com/openbook-dex/program/blob/master/dex/crank/src/lib.rs#L349) to create a new market.

2. Add dependencies in your Cargo.toml
```rust
[dependencies]
[features]
# default is mainnet
devnet = [
    "amm-cli/devnet",
    "common/devnet",
]

[dependencies]
amm-cli = { git = "https://github.com/raydium-io/raydium-library" }
common = { git = "https://github.com/raydium-io/raydium-library" }
spl-token = { version = "4.0.0", features = ["no-entrypoint"] }
spl-associated-token-account = { version = "2.2.0", features = [
    "no-entrypoint",
] }
spl-token-2022 = { version = "0.9.0", features = ["no-entrypoint"] }
solana-client = "<1.17.0"
solana-sdk = "<1.17.0"
anyhow = "1.0.53"
clap = { version = "4.1.8", features = ["derive"] }
```

3. Import dependent libraries
```rust
#![allow(dead_code)]
use anyhow::{Ok, Result};
use clap::Parser;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{commitment_config::CommitmentConfig, signer::Signer};
use std::sync::Arc;

use {
    amm_cli::{self, AmmCommands},
    common::{common_types, common_utils, rpc},
};
```

4. Custom configuration parameters in your code.
```rust
// default config
let mut config = common_types::CommonConfig::default();
// Replace the default configuration parameters you need
config.set_cluster("http", "ws");
config.set_wallet("your wallet path");
config.set_amm_program("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");
config.set_openbook_program("srmqPvymJeFKQ4zGQed1GFppgkRHL9kaELCbyksJtPX");
config.set_slippage(50);
```

5. Constructing a signed storage object.
```rust
let payer = common_utils::read_keypair_file(&config.wallet())?;
let fee_payer = payer.pubkey();
let mut signing_keypairs: Vec<Arc<dyn Signer>> = Vec::new();
let payer: Arc<dyn Signer> = Arc::new(payer);
if !signing_keypairs.contains(&payer) {
    signing_keypairs.push(payer);
}
```

6. initialize a new amm pool with an associate openbook market
```rust
// build initialize pool instruction
let subcmd = AmmCommands::CreatePool {
    market: Pubkey::from_str("The amm associated with openbook market").unwrap(),
    coin_mint: Pubkey::from_str("The openbook market's coin_mint").unwrap(),
    pc_mint: Pubkey::from_str("The openbook market's pc_mint").unwrap(),
    user_token_coin: Pubkey::from_str("User's token coin").unwrap(),
    user_token_pc: Pubkey::from_str("User's token pc").unwrap(),
    init_coin_amount: 100000u64,
    init_pc_amount: 100000u64,
    open_time: 0,
};
let instruction = amm_cli::process_amm_commands(subcmd, &config).unwrap();
```

3. deposit assets to an amm pool
```rust
// build deposit instruction
let subcmd = AmmCommands::Deposit {
    pool_id: Pubkey::from_str("The specified pool of the assets deposite to").unwrap(),
    deposit_token_coin: Some(Pubkey::from_str("The specified token coin of the user deposit").unwrap()),
    deposit_token_pc: Some(Pubkey::from_str("The specified token pc of the user deposit").unwrap()),
    recipient_token_lp: Some(Pubkey::from_str("The specified lp token of the user will receive").unwrap()),
    amount_specified: 100000u64,
    another_min_limit: false,
    base_coin: false,
};
let instruction = amm_cli::process_amm_commands(subcmd, &config).unwrap();
```
### Note
If the parameter of the deposit_token_coin, deposit_token_pc or recipient_token_lp is None, it will be ATA token by default.

4. withdraw assets from amm pool
```rust
// build withdraw instruction
let subcmd = AmmCommands::Withdraw {
    pool_id: Pubkey::from_str("The specified pool of the assets withdraw from").unwrap(),
    withdraw_token_lp: Some(Pubkey::from_str("The specified lp token of the user withdraw").unwrap()),
    recipient_token_coin: Some(Pubkey::from_str("The specified token coin of the user will receive").unwrap()),
    recipient_token_pc: Some(Pubkey::from_str("The specified token pc of the user will receive").unwrap()),
    input_lp_amount: 100000u64,
    slippage_limit: false,
};
let instruction = amm_cli::process_amm_commands(subcmd, &config).unwrap();
```
### Note
If the parameter of the withdraw_token_lp, recipient_token_coin or recipient_token_pc is None, it will be ATA token by default.

5. swap
```rust
// build swap instruction
let subcmd = AmmCommands::Swap {
    pool_id: Pubkey::from_str(" The specified pool of trading").unwrap(),
    user_input_token: Pubkey::from_str("The token of user want to swap from").unwrap(),
    user_output_token: Some(Pubkey::from_str("The token of user want to swap to").unwrap()),
    amount_specified: 100000u64,
    base_out: false,
};
let instruction = amm_cli::process_amm_commands(subcmd, &config).unwrap();
```
### Note
If the parameter of the user_output_token is None, it will be ATA token by default.

For more information, you can see the repo [raydium-library](https://github.com/raydium-io/raydium-library)
