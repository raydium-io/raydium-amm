<div align="center">
  <h1>raydium-amm</h1>
</div>

## Program Deployments

| Environment         |   [PROGRAM](/program)                          |
| ------------------- | ---------------------------------------------- |
| Mainnet             | `675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8` |
| Devnet              | `HWy1jotHpo6UqeQxx49dpYYdQB8wj9Qk9MdxwjLvDHB8` |

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
cargo build-bpf
```
### Devnet Build
```bash
cargo build-bpf --features devnet
```
### Localnet Build
You must update these pubkeys in the "config_feature" as yours over the localnet feature before build;

```bash
cargo build-bpf --features localnet
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
raydium-library = { git = "https://github.com/raydium-io/raydium-library" }
spl-token = { version = "4.0.0", features = ["no-entrypoint"] }
spl-associated-token-account = { version = "2.2.0", features = [
    "no-entrypoint",
] }
spl-token-2022 = { version = "0.9.0", features = ["no-entrypoint"] }
solana-client = "<1.17.0"
solana-sdk = "<1.17.0"
anyhow = "1.0.53"
```

3. Import dependent libraries
```rust
#![allow(dead_code)]
use anyhow::{format_err, Result};
use raydium_library::amm;
use std::str::FromStr;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction, pubkey::Pubkey, signature::Signer,
    transaction::Transaction,
};
```

2. initialize a new amm pool with an associate openbook market
```rust
// config params
let wallet_file_path = "id.json";
let cluster_url = "https://api.devnet.solana.com/";
let amm_program = Pubkey::from_str("HWy1jotHpo6UqeQxx49dpYYdQB8wj9Qk9MdxwjLvDHB8")?;
let market_program = Pubkey::from_str("EoTcMgcDRTJVZDMZWBoU6rhYHZfkNTVEAfz3uUJRcYGj")?;
let market = Pubkey::from_str("74yqm5ihhMg5XJeqvC6oPsHaczjF6U9Rc8zs4wMnAGUL")?;
let amm_coin_mint = Pubkey::from_str("2SiSpNowr7zUv5ZJHuzHszskQNaskWsNukhivCtuVLHo")?;
let amm_pc_mint = Pubkey::from_str("GfmdKWR1KrttDsQkJfwtXovZw9bUBHYkPAEwB6wZqQvJ")?;
// maintnet: 7YttLkHDoNj9wyDur5pM1ejNaAvT9X4eqaYcHQqtj2G5
// devnet: 3XMrhbv989VxAMi3DErLV9eJht1pHppW5LbKxe9fkEFR
let create_fee_destination = Pubkey::from_str("3XMrhbv989VxAMi3DErLV9eJht1pHppW5LbKxe9fkEFR")?;

let client = RpcClient::new(cluster_url.to_string());
let wallet = solana_sdk::signature::read_keypair_file(wallet_file_path)
    .map_err(|_| format_err!("failed to read keypair from {}", wallet_file_path))?;

let input_pc_amount = 10000_000000;
let input_coin_amount = 10000_000000;
// generate amm keys
let amm_keys = raydium_library::amm::utils::get_amm_pda_keys(
    &amm_program,
    &market_program,
    &market,
    &amm_coin_mint,
    &amm_pc_mint,
)?;
// build initialize instruction
let build_init_instruction = raydium_library::amm::commands::initialize_amm_pool(
    &amm_program,
    &amm_keys,
    &create_fee_destination,
    &wallet.pubkey(),
    &spl_associated_token_account::get_associated_token_address(
        &wallet.pubkey(),
        &amm_keys.amm_coin_mint,
    ),
    &spl_associated_token_account::get_associated_token_address(
        &wallet.pubkey(),
        &amm_keys.amm_pc_mint,
    ),
    &spl_associated_token_account::get_associated_token_address(
        &wallet.pubkey(),
        &amm_keys.amm_lp_mint,
    ),
    0,
    input_pc_amount,
    input_coin_amount,
)?;
```

3. deposit assets to an amm pool
```rust
// config params
let wallet_file_path = "id.json";
let cluster_url = "https://api.devnet.solana.com/";
let amm_program = Pubkey::from_str("HWy1jotHpo6UqeQxx49dpYYdQB8wj9Qk9MdxwjLvDHB8")?;
let amm_pool_id = Pubkey::from_str("BbZjQanvSaE9me4adAitmTTaSgASvzaVignt4HRSM7ww")?;
let slippage_bps = 50u64; // 0.5%
let input_amount = 10000_000000;
let base_side = 0; // 0: base coin; 1: base pc

let client = RpcClient::new(cluster_url.to_string());
let wallet = solana_sdk::signature::read_keypair_file(wallet_file_path)
    .map_err(|_| format_err!("failed to read keypair from {}", wallet_file_path))?;

// load amm keys
let amm_keys = raydium_library::amm::utils::load_amm_keys(&client, &amm_program, &amm_pool_id)?;
// load market keys
let market_keys = raydium_library::amm::openbook::get_keys_for_market(
    &client,
    &amm_keys.market_program,
    &amm_keys.market,
)?;
// calculate amm pool vault with load data at the same time or use simulate to calculate
let result = raydium_library::amm::calculate_pool_vault_amounts(
    &client,
    &amm_program,
    &amm_pool_id,
    &amm_keys,
    &market_keys,
    amm::utils::CalculateMethod::Simulate,
    Some(&wallet.pubkey()),
)?;
// calculate amounts
let (max_coin_amount, max_pc_amount) =
    raydium_library::amm::amm_math::deposit_amount_with_slippage(
        result.pool_pc_vault_amount,
        result.pool_coin_vault_amount,
        input_amount,
        base_side,
        slippage_bps,
    )?;
// build deposit instruction
let build_deposit_instruction = raydium_library::amm::commands::deposit(
    &amm_program,
    &amm_keys,
    &market_keys,
    &wallet.pubkey(),
    &spl_associated_token_account::get_associated_token_address(
        &wallet.pubkey(),
        &amm_keys.amm_coin_mint,
    ),
    &spl_associated_token_account::get_associated_token_address(
        &wallet.pubkey(),
        &amm_keys.amm_pc_mint,
    ),
    &spl_associated_token_account::get_associated_token_address(
        &wallet.pubkey(),
        &amm_keys.amm_lp_mint,
    ),
    max_coin_amount,
    max_pc_amount,
    base_side,
)?;
```

4. withdraw assets from amm pool
```rust
// config params
let wallet_file_path = "id.json";
let cluster_url = "https://api.devnet.solana.com/";
let amm_program = Pubkey::from_str("HWy1jotHpo6UqeQxx49dpYYdQB8wj9Qk9MdxwjLvDHB8")?;
let amm_pool_id = Pubkey::from_str("BbZjQanvSaE9me4adAitmTTaSgASvzaVignt4HRSM7ww")?;
// let slippage_bps = 50u64; // 0.5%
let withdraw_lp_amount = 10000_000000;

let client = RpcClient::new(cluster_url.to_string());
let wallet = solana_sdk::signature::read_keypair_file(wallet_file_path)
    .map_err(|_| format_err!("failed to read keypair from {}", wallet_file_path))?;

// load amm keys
let amm_keys = raydium_library::amm::utils::load_amm_keys(&client, &amm_program, &amm_pool_id)?;
// load market keys
let market_keys = raydium_library::amm::openbook::get_keys_for_market(
    &client,
    &amm_keys.market_program,
    &amm_keys.market,
)?;
// build withdraw instruction
let build_withdraw_instruction = raydium_library::amm::commands::withdraw(
    &amm_program,
    &amm_keys,
    &market_keys,
    &wallet.pubkey(),
    &spl_associated_token_account::get_associated_token_address(
        &wallet.pubkey(),
        &amm_keys.amm_coin_mint,
    ),
    &spl_associated_token_account::get_associated_token_address(
        &wallet.pubkey(),
        &amm_keys.amm_pc_mint,
    ),
    &spl_associated_token_account::get_associated_token_address(
        &wallet.pubkey(),
        &amm_keys.amm_lp_mint,
    ),
    withdraw_lp_amount,
)?;
```

5. swap
```rust
// config params
let wallet_file_path = "id.json";
let cluster_url = "https://api.devnet.solana.com/";
let amm_program = Pubkey::from_str("HWy1jotHpo6UqeQxx49dpYYdQB8wj9Qk9MdxwjLvDHB8")?;
let amm_pool_id = Pubkey::from_str("BbZjQanvSaE9me4adAitmTTaSgASvzaVignt4HRSM7ww")?;
let input_token_mint = Pubkey::from_str("GfmdKWR1KrttDsQkJfwtXovZw9bUBHYkPAEwB6wZqQvJ")?;
let output_token_mint = Pubkey::from_str("2SiSpNowr7zUv5ZJHuzHszskQNaskWsNukhivCtuVLHo")?;
let slippage_bps = 50u64; // 0.5%
let amount_specified = 2000_000000u64;
let swap_base_in = false;

let client = RpcClient::new(cluster_url.to_string());
let wallet = solana_sdk::signature::read_keypair_file(wallet_file_path)
    .map_err(|_| format_err!("failed to read keypair from {}", wallet_file_path))?;

// load amm keys
let amm_keys = raydium_library::amm::utils::load_amm_keys(&client, &amm_program, &amm_pool_id)?;
// load market keys
let market_keys = raydium_library::amm::openbook::get_keys_for_market(
    &client,
    &amm_keys.market_program,
    &amm_keys.market,
)?;
// calculate amm pool vault with load data at the same time or use simulate to calculate
let result = raydium_library::amm::calculate_pool_vault_amounts(
    &client,
    &amm_program,
    &amm_pool_id,
    &amm_keys,
    &market_keys,
    amm::utils::CalculateMethod::Simulate,
    Some(&wallet.pubkey()),
)?;
let direction = if input_token_mint == amm_keys.amm_coin_mint
    && output_token_mint == amm_keys.amm_pc_mint
{
    amm::utils::SwapDirection::Coin2PC
} else {
    amm::utils::SwapDirection::PC2Coin
};
let other_amount_threshold = raydium_library::amm::swap_with_slippage(
    result.pool_pc_vault_amount,
    result.pool_coin_vault_amount,
    result.swap_fee_numerator,
    result.swap_fee_denominator,
    direction,
    amount_specified,
    swap_base_in,
    slippage_bps,
)?;
// build swap instruction
let build_swap_instruction = raydium_library::amm::commands::swap(
    &amm_program,
    &amm_keys,
    &market_keys,
    &wallet.pubkey(),
    &spl_associated_token_account::get_associated_token_address(
        &wallet.pubkey(),
        &input_token_mint,
    ),
    &spl_associated_token_account::get_associated_token_address(
        &wallet.pubkey(),
        &output_token_mint,
    ),
    amount_specified,
    other_amount_threshold,
    swap_base_in,
)?;
```
