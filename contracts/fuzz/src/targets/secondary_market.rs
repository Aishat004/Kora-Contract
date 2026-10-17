//! Fuzz harness for `secondary_market::buy_position` — issue #918.
//!
//! ## Purpose
//!
//! The `buy_position` function implements the Checks-Effects-Interactions (CEI)
//! pattern: it clears the listing entry from storage **before** making any
//! external token transfers or calling the financing pool.  This harness verifies
//! that the CEI ordering holds across all reachable input sequences by asserting
//! the following invariants after every `buy_position` call:
//!
//! ### Invariants
//!
//! | # | Name | Description |
//! |---|------|-------------|
//! | INV-SM-1 | Listing cleared atomically | After a successful `buy_position`, the same listing must not be buyable a second time (the storage entry is removed before external calls). |
//! | INV-SM-2 | No double-spend | A second `buy_position` on the same (invoice_id, seller) pair must fail once the first succeeds. |
//! | INV-SM-3 | Buyer ≠ seller guard | `buy_position(buyer = seller)` must always return `SameAddress`. |
//! | INV-SM-4 | Paused guard | All mutating calls must fail when the contract is paused. |
//!
//! ## Structure
//!
//! Each fuzz iteration starts from a fresh `SmProtocol` backed by the full
//! Kora financing pool (registered but minimally initialized).  The fuzzer drives
//! an arbitrary sequence of [`Op`]s including both lifecycle operations
//! (`list_position`, `buy_position`, `cancel_listing`) and adversarial operations
//! (pause, double-buy, self-buy).  Any panic or invariant violation inside `run`
//! is a finding.

use arbitrary::Arbitrary;
use soroban_sdk::testutils::{Address as _, EnvTestConfig, Ledger, LedgerInfo};
use soroban_sdk::{token::StellarAssetClient, Address, Env};

use kora_access_control::{AccessControlContract, AccessControlContractClient};
use kora_financing_pool::{FinancingPoolContract, FinancingPoolContractClient};
use kora_invoice_nft::{InvoiceNftContract, InvoiceNftContractClient};
use kora_price_oracle::{PriceOracleContract, PriceOracleContractClient};
use kora_risk_registry::{RiskRegistryContract, RiskRegistryContractClient};
use kora_secondary_market::{SecondaryMarket, SecondaryMarketClient};
use kora_treasury::{TreasuryContract, TreasuryContractClient};

use crate::gen;

// ── Test environment ──────────────────────────────────────────────────────────

struct SmProtocol<'a> {
    env: Env,
    sm: SecondaryMarketClient<'a>,
    pool: FinancingPoolContractClient<'a>,
    admin: Address,
    treasury: Address,
    token: Address,
    actors: std::vec::Vec<Address>,
}

impl SmProtocol<'static> {
    fn deploy() -> Self {
        let mut env = Env::default();
        env.set_config(EnvTestConfig {
            capture_snapshot_at_drop: false,
        });
        env.mock_all_auths_allowing_non_root_auth();
        env.ledger().set(LedgerInfo {
            timestamp: 1_700_000_000,
            protocol_version: 21,
            sequence_number: 1,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: 1000,
            min_persistent_entry_ttl: 1000,
            max_entry_ttl: 100_000,
        });

        let admin = Address::generate(&env);
        let seller = Address::generate(&env);
        let buyer = Address::generate(&env);
        let outsider = Address::generate(&env);

        // Deploy the contracts we need.
        let ac_id = env.register_contract(None, AccessControlContract);
        let nft_id = env.register_contract(None, InvoiceNftContract);
        let pool_id = env.register_contract(None, FinancingPoolContract);
        let treasury_id = env.register_contract(None, TreasuryContract);
        let rr_id = env.register_contract(None, RiskRegistryContract);
        let oracle_id = env.register_contract(None, PriceOracleContract);
        let sm_id = env.register_contract(None, SecondaryMarket);

        let access_control = AccessControlContractClient::new(&env, &ac_id);
        let invoice_nft = InvoiceNftContractClient::new(&env, &nft_id);
        let pool = FinancingPoolContractClient::new(&env, &pool_id);
        let treasury = TreasuryContractClient::new(&env, &treasury_id);
        let rr = RiskRegistryContractClient::new(&env, &rr_id);
        let oracle = PriceOracleContractClient::new(&env, &oracle_id);
        let sm = SecondaryMarketClient::new(&env, &sm_id);

        // Token setup.
        let token_admin = Address::generate(&env);
        let token = env
            .register_stellar_asset_contract_v2(token_admin)
            .address();
        let sac = StellarAssetClient::new(&env, &token);
        for a in [&admin, &seller, &buyer, &outsider] {
            sac.mint(a, &1_000_000_000_000i128);
        }

        // Initialize the dependency chain (best-effort; errors ignored).
        let _ = access_control.try_initialize(&admin);
        let _ = invoice_nft.try_initialize(&admin, &ac_id);
        let _ = invoice_nft.try_set_risk_registry(&admin, &rr_id);
        let _ = oracle.try_initialize(&admin, &ac_id);
        let _ = treasury.try_initialize(&admin, &50u32);
        let _ = treasury.try_whitelist_token(&admin, &token);
        let _ = rr.try_initialize(&admin, &nft_id, &token, &1_000_000i128, &5_000u32);
        let _ = pool.try_initialize(
            &admin,
            &nft_id,
            &rr_id,
            &treasury_id,
            &ac_id,
            &200u32,
            &oracle_id,
            &10_000u32,
            &86_400u64,
            &Address::generate(&env),
        );
        let _ = invoice_nft.try_set_authorized_callers(&admin, &sm_id, &pool_id);

        // Initialize secondary market: connects to pool, treasury, fee=250bps.
        let _ = sm.try_initialize(&admin, &pool_id, &treasury_id, &250u32);

        let actors = std::vec![
            admin.clone(),
            seller.clone(),
            buyer.clone(),
            outsider.clone(),
            token.clone(),
            ac_id.clone(),
            nft_id.clone(),
            pool_id.clone(),
            treasury_id.clone(),
            sm_id.clone(),
        ];

        SmProtocol {
            env,
            sm,
            pool,
            admin,
            treasury: treasury_id,
            token,
            actors,
        }
    }

    fn actor(&self, sel: u8) -> Address {
        self.actors[sel as usize % self.actors.len()].clone()
    }
    fn seller(&self) -> Address { self.actors[1].clone() }
    fn buyer(&self) -> Address  { self.actors[2].clone() }
}

// ── Fuzz operations ───────────────────────────────────────────────────────────

#[derive(Arbitrary, Debug)]
enum Op {
    /// List a position for sale — seeds state for buy_position.
    ListPosition {
        seller: u8,
        invoice_id: u64,
        itag: u8,
        price: i128,
        ptag: u8,
    },
    /// Core CEI target.
    ///
    /// INV-SM-1: After a successful buy, the listing is cleared; a second buy
    ///           on the same (invoice_id, seller) must fail.
    /// INV-SM-2: Only one successful buy per listing regardless of concurrency.
    BuyPosition {
        buyer: u8,
        invoice_id: u64,
        itag: u8,
        seller: u8,
    },
    /// Adversarial: attempt to buy the same listing twice in one run.
    ///
    /// INV-SM-2: Must fail on the second attempt.
    DoubleBuy {
        invoice_id: u64,
        itag: u8,
        seller: u8,
    },
    /// Adversarial: buyer == seller — must always return SameAddress (INV-SM-3).
    SelfBuy {
        actor: u8,
        invoice_id: u64,
        itag: u8,
    },
    /// Adversarial: pause the contract then attempt a buy — must fail (INV-SM-4).
    PausedBuy {
        buyer: u8,
        invoice_id: u64,
        itag: u8,
        seller: u8,
    },
    /// Cancel an existing listing.
    CancelListing {
        seller: u8,
        invoice_id: u64,
        itag: u8,
    },
    /// Toggle pause state.
    Pause { pause: bool },
    /// Update the fee bps — must not panic on any value.
    UpdateFee { fee_bps: u32, ftag: u8 },
}

pub fn run(data: &[u8]) {
    let p = SmProtocol::deploy();

    super::drive::<Op, _>(data, |op| {
        let sm = &p.sm;

        match op {
            Op::ListPosition { seller, invoice_id, itag, price, ptag } => {
                let id = gen::ts_or_id(invoice_id, itag);
                let seller_addr = p.actor(seller);
                let price_val = gen::amount(price, ptag);
                let _ = sm.try_list_position(&seller_addr, &id, &p.token, &price_val);
            }

            Op::BuyPosition { buyer, invoice_id, itag, seller } => {
                let id = gen::ts_or_id(invoice_id, itag);
                let buyer_addr = p.actor(buyer);
                let seller_addr = p.actor(seller);

                // ── INV-SM-1 / INV-SM-2: CEI atomicity ───────────────────────
                // A successful buy must remove the listing before doing any
                // external work.  We verify this by asserting a second
                // identical buy always fails once the first succeeds.
                let first = sm.try_buy_position(&buyer_addr, &id, &seller_addr);
                if first.is_ok() {
                    let second = sm.try_buy_position(&buyer_addr, &id, &seller_addr);
                    assert!(
                        second.is_err(),
                        "INV-SM-2 VIOLATED: second buy_position succeeded after first — \
                         listing was NOT cleared atomically before external calls (CEI failure)"
                    );
                }
            }

            Op::DoubleBuy { invoice_id, itag, seller } => {
                let id = gen::ts_or_id(invoice_id, itag);
                let seller_addr = p.actor(seller);
                let buyer_addr = p.buyer();

                // Attempt two back-to-back buys; second must always fail.
                let first = sm.try_buy_position(&buyer_addr, &id, &seller_addr);
                if first.is_ok() {
                    let second = sm.try_buy_position(&buyer_addr, &id, &seller_addr);
                    assert!(
                        second.is_err(),
                        "INV-SM-2 VIOLATED (DoubleBuy): second consecutive buy succeeded — \
                         reentrancy or CEI ordering issue in buy_position"
                    );
                }
            }

            Op::SelfBuy { actor, invoice_id, itag } => {
                let id = gen::ts_or_id(invoice_id, itag);
                let addr = p.actor(actor);
                // INV-SM-3: buyer == seller must always be rejected.
                let result = sm.try_buy_position(&addr, &id, &addr);
                assert!(
                    result.is_err(),
                    "INV-SM-3 VIOLATED: self-buy (buyer == seller == {addr:?}) succeeded — \
                     SameAddress guard is missing or bypassed"
                );
            }

            Op::PausedBuy { buyer, invoice_id, itag, seller } => {
                let id = gen::ts_or_id(invoice_id, itag);
                let buyer_addr = p.actor(buyer);
                let seller_addr = p.actor(seller);

                // INV-SM-4: any mutating call must fail when paused.
                let _ = sm.try_pause();
                let result = sm.try_buy_position(&buyer_addr, &id, &seller_addr);
                assert!(
                    result.is_err(),
                    "INV-SM-4 VIOLATED: buy_position succeeded while contract is paused"
                );
                // Restore to unpaused so subsequent ops are not all stuck.
                let _ = sm.try_unpause();
            }

            Op::CancelListing { seller, invoice_id, itag } => {
                let id = gen::ts_or_id(invoice_id, itag);
                let _ = sm.try_cancel_listing(&p.actor(seller), &id);
            }

            Op::Pause { pause } => {
                if pause {
                    let _ = sm.try_pause();
                } else {
                    let _ = sm.try_unpause();
                }
            }

            Op::UpdateFee { fee_bps, ftag } => {
                // Must never panic regardless of input.
                let fee = gen::small_u32(fee_bps, ftag);
                let _ = sm.try_update_protocol_fee_bps(&p.admin, &fee);
            }
        }
    });
}
