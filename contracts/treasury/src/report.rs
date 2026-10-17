//! Treasury Transparency Report Generator
//! Generates periodic reports summarizing treasury inflows, outflows, and ending balances.

use soroban_sdk::{contracttype, Address, Env, Vec};

/// Token-specific treasury data for a report entry.
#[contracttype]
#[derive(Clone, Debug)]
pub struct TokenBalance {
    pub token: Address,
    /// Number of decimal places used to convert raw amounts into human units.
    pub decimals: u32,
    pub inflows: i128,
    pub outflows: i128,
    pub ending_balance: i128,
}

/// A comprehensive treasury report for a given epoch.
/// Summarizes inflows (fees by source), outflows (withdrawals), and ending balance.
#[contracttype]
#[derive(Clone, Debug)]
pub struct TreasuryReport {
    /// Epoch identifier (timestamp or epoch number).
    pub epoch: u64,
    /// Per-token inflows, outflows, and balances.
    pub balances: Vec<TokenBalance>,
    /// Report generation timestamp.
    pub generated_at: u64,
}

impl TreasuryReport {
    /// Create a new treasury report for the given epoch.
    pub fn new(env: &Env, epoch: u64) -> Self {
        TreasuryReport {
            epoch,
            balances: Vec::new(env),
            generated_at: env.ledger().timestamp(),
        }
    }

    /// Add or update a token entry in the report.
    pub fn set_token_balance(
        &mut self,
        token: Address,
        decimals: u32,
        inflows: i128,
        outflows: i128,
        ending_balance: i128,
    ) {
        // Find and update existing entry, or append new one
        let mut found = false;
        for i in 0..self.balances.len() {
            if let Some(entry) = self.balances.get(i) {
                if entry.token == token {
                    self.balances.set(i, TokenBalance {
                        token: token.clone(),
                        decimals,
                        inflows,
                        outflows,
                        ending_balance,
                    });
                    found = true;
                    break;
                }
            }
        }
        if !found {
            self.balances.push_back(TokenBalance {
                token,
                decimals,
                inflows,
                outflows,
                ending_balance,
            });
        }
    }

    /// Get the number of tokens in the report.
    pub fn token_count(&self) -> u32 {
        self.balances.len() as u32
    }
}
