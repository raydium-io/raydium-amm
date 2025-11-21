// lib.rs - Raydium AMM Program Core

// #![deny(missing_docs)] // Commented out by original author, indicating documentation is not strictly enforced.

//! An Uniswap-like program for the Solana blockchain.
//! This program implements the Automated Market Maker (AMM) logic for trading tokens.

#[macro_use]
// Public logging utilities for debugging and tracing.
pub mod log; 

// =======================================================
// CORE PROGRAM MODULES
// =======================================================

mod entrypoint;      // The main entry point for the Solana BPF/SBF VM.
pub mod error;       // Custom program error types.
pub mod instruction; // Defines data structures for external program instructions (API).
pub mod invokers;    // Utility for invoking other programs (CPI helpers).
pub mod math;        // Contains safe arithmetic functions for financial operations.
pub mod processor;   // The main logic handler for processing instructions.
pub mod state;       // Defines all necessary account state structures (e.g., Pool state).

// Export current solana-program types for downstream users 
// who may also be building with a different solana-sdk version, ensuring compatibility.
pub use solana_program;

// =======================================================
// SECURITY & DEPLOYMENT CONFIGURATION
// =======================================================

#[cfg(not(feature = "no-entrypoint"))]
// Embeds a standardized security.txt file into the program binary.
// This is critical for vulnerability disclosure compliance (VDP).
solana_security_txt::security_txt! {
    name: "raydium-amm",
    project_url: "https://raydium.io",
    contacts: "link:https://immunefi.com/bounty/raydium",
    policy: "https://immunefi.com/bounty/raydium",
    source_code: "https://github.com/raydium-io/raydium-amm",
    preferred_languages: "en",
    auditors: "https://github.com/raydium-io/raydium-docs/blob/master/audit/MadSheild%20Q2%202023/Raydium%20updated%20orderbook%20AMM%20program%20&%20OpenBook%20migration.pdf"
}

// =======================================================
// PROGRAM ID DECLARATION
// =======================================================

#[cfg(feature = "devnet")]
// Program ID for the Devnet deployment.
solana_program::declare_id!("DRaya7Kj3aMWQSy19kSjvmuwq9docCHofyP9kanQGaav");

#[cfg(not(feature = "devnet"))]
// Program ID for the Mainnet Beta deployment (Production).
solana_program::declare_id!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");
