# Access Control Matrix

**Last Updated:** 2026-09-28  
**Purpose:** Comprehensive authorization matrix mapping every public entrypoint to required roles/permissions  
**Automated Checker:** `contracts/xtask/src/bin/check_access_matrix.rs`

## Overview

This document serves as the single source of truth for access control across all Kora smart contracts. Every public entrypoint is mapped to its required authorization, and an automated checker verifies that the implementation matches this specification.

## Role Definitions

### System Roles

| Role | Description | Granted By |
|------|-------------|------------|
| **Admin** | Protocol administrator with upgrade and configuration privileges | `access_control` multisig or single admin |
| **Verifier** | KYB/risk-assessment provider authorized to attest SME profiles | `risk_registry.register_verifier` (admin-only) |
| **Feeder** | Price oracle data provider authorized to submit exchange rates | `price_oracle.add_feeder` (admin-only) |
| **SME** | Small/medium enterprise seeking invoice financing | Self-registered via `risk_registry.register_sme` |
| **Investor** | Capital provider funding invoice listings | Any address (permissionless funding) |
| **Governance** | Multi-sig or DAO contract for dispute resolution | Configured in `dispute_resolution` |

### Special Authorization Patterns

| Pattern | Description | Check Method |
|---------|-------------|--------------|
| **Self** | Caller must be the resource owner (e.g., SME calling on their own invoice) | Direct address comparison |
| **Contract-to-Contract** | Authorized cross-contract caller (e.g., `marketplace` calling `financing_pool`) | Storage-configured contract address |
| **Conditional** | Authorization depends on state (e.g., only if not paused) | Multiple checks combined with `&&` |
| **Permissionless** | No authorization required (read-only views) | No `require_auth()` call |

---

## Access Matrix by Contract

### 1. access_control

| Entrypoint | Required Role | Conditional Authorization | Notes |
|------------|---------------|---------------------------|-------|
| `initialize` | None (one-time) | Must not already be initialized | Sets initial admin |
| `set_admin` | **Admin** | Requires multisig quorum if threshold >1 | |
| `propose_admin` | **Admin** | Multisig quorum required | Two-step admin transfer |
| `accept_admin` | **PendingAdmin** | Caller must be the proposed new admin | |
| `set_protocol_paused` | **Admin** | Multisig quorum required | Emergency pause |
| `is_protocol_paused` | Permissionless | Read-only | |
| `add_multisig_signer` | **Admin** | Multisig quorum required | |
| `remove_multisig_signer` | **Admin** | Multisig quorum required | Cannot remove below threshold |
| `set_multisig_threshold` | **Admin** | Multisig quorum required | threshold ≤ signer count |
| `get_multisig_signers` | Permissionless | Read-only | |
| `get_multisig_threshold` | Permissionless | Read-only | |
| `record_admin_action` | **Admin** OR authorized contract | Used by other contracts to log admin actions | Audit trail |
| `get_admin_audit_log` | Permissionless | Read-only | Returns last N admin actions |

**Authorization Checker:**
```rust
fn require_admin(&env, caller: &Address) -> Result<(), AccessControlError> {
    let admin = env.storage().instance().get(&DataKey::Admin).unwrap();
    if caller != &admin { return Err(AccessControlError::NotAdmin); }
    // If multisig configured, verify quorum via approve_pending_action
    Ok(())
}
```

---

### 2. risk_registry

| Entrypoint | Required Role | Conditional Authorization | Notes |
|------------|---------------|---------------------------|-------|
| `initialize` | None (one-time) | Must not already be initialized | |
| `register_sme` | **Self (SME)** | Caller authorizes; verifier attests separately | Permissionless registration |
| `update_sme_profile` | **Verifier** (assigned to SME) | Must be SME's verifier-of-record | |
| `register_verifier` | **Admin** | | Requires staking token transfer |
| `remove_verifier` | **Admin** | | Returns stake; slashing may apply |
| `set_debtor_score` | **Verifier** | MIN_SCORE_UPDATE_INTERVAL cooldown | Max 50 verifiers per debtor |
| `get_debtor_score` | Permissionless | Read-only | Aggregates from active verifiers |
| `set_credit_limit` | **Verifier** (assigned to SME) | Must be SME's verifier-of-record | |
| `increment_invoice_count` | **InvoiceNft Contract** | Caller must be registered invoice_nft address | Cross-contract |
| `slash_verifier` | **Admin** | | Penalty for missed default predictions |
| `get_sme_profile` | Permissionless | Read-only | |
| `is_verifier` | Permissionless | Read-only | |

**Special Authorizations:**
- `SubAccount(address)` → allows delegate addresses to act on behalf of primary verifier
- Verifier-of-record: Each SME has an assigned verifier; only that verifier can update the SME's profile

---

### 3. invoice_nft

| Entrypoint | Required Role | Conditional Authorization | Notes |
|------------|---------------|---------------------------|-------|
| `initialize` | None (one-time) | | |
| `mint_invoice` | **SME (Self)** | Rate-limited by mint_rate_limit | |
| `mint_invoices_batch` | **SME (Self)** | Batch size ≤ MAX_BATCH_MINT_SIZE (25) | Rate limit applies per-invoice |
| `set_funded` | **Marketplace Contract** | Caller must be registered marketplace address | |
| `set_repaid` | **FinancingPool Contract** | Caller must be registered financing_pool address | |
| `set_defaulted` | **Admin** | | Manual default marking |
| `freeze_invoice` | **Admin** | | Blocks further state transitions |
| `unfreeze_invoice` | **Admin** | | |
| `set_mint_rate_limit` | **Admin** | | Anti-spam protection |
| `get_invoice` | Permissionless | Read-only | |
| `get_sme_invoices` | Permissionless | Read-only | Returns invoice IDs for SME |
| `is_invoice_frozen` | Permissionless | Read-only | |

**State Transition Authorization:**
- Only marketplace can transition `Created → Funded`
- Only financing_pool can transition `Funded → Repaid`
- Only admin can transition any state → `Defaulted`

---

### 4. marketplace

| Entrypoint | Required Role | Conditional Authorization | Notes |
|------------|---------------|---------------------------|-------|
| `initialize` | None (one-time) | | |
| `set_fee_bps` | **Admin** | Multisig quorum if configured | |
| `set_referrer_split_bps` | **Admin** | Multisig quorum if configured | |
| `set_min_contribution` | **Admin** | | DoS protection for listings |
| `set_max_investor_share_bps` | **Admin** | | Concentration cap per investor |
| `set_investor_accredited` | **Admin** | | KYC/accreditation flag |
| `set_tier_fee_bps` | **Admin** | | Risk-tier-specific fee override |
| `propose_token_whitelist` | **Admin** | Starts timelock | |
| `execute_token_whitelist` | **Admin** | Timelock elapsed | |
| `remove_token_whitelist` | **Admin** | Immediate (no timelock) | |
| `set_token_currency` | **Admin** | | Maps token → oracle symbol |
| `set_token_exposure_cap` | **Admin** | | Global limit per token |
| `set_priority_window` | **Admin** | | Whitelist-only funding period |
| `list_invoice` | **SME (invoice owner)** | Invoice in `Created` state | |
| `fund_invoice` | **Investor (Self)** | Not paused; not in priority window OR investor whitelisted | Concentration cap enforced |
| `fund_invoices_batch` | **Investor (Self)** | Same as fund_invoice per listing | |
| `cancel_listing` | **SME (invoice owner)** | No funding yet (funded_amount == 0) | |
| `withdraw_listing` | **SME (invoice owner)** | No funding yet | Alias for cancel_listing |
| `request_cancellation` | **SME (invoice owner)** | Partially funded; requires admin confirmation | |
| `admin_confirm_cancellation` | **Admin** | Pending cancellation request exists | Enables investor refunds |
| `claim_refund` | **Investor (Self)** | Cancellation confirmed OR deadline passed unfunded | |
| `set_discount_rate_override` | **Admin** | | Custom APY per listing |
| `remove_discount_rate_override` | **Admin** | | Revert to default APY calculation |
| `get_listing` | Permissionless | Read-only | |
| `get_config` | Permissionless | Read-only | |
| `is_token_whitelisted` | Permissionless | Read-only | |

**Cross-Currency Funding:**
- `fund_invoice` converts investor token → listing token via `price_oracle`
- Authorization unchanged; oracle manipulation defenses apply (see ORACLE_ADVERSARIAL_TESTING.md)

---

### 5. financing_pool

| Entrypoint | Required Role | Conditional Authorization | Notes |
|------------|---------------|---------------------------|-------|
| `initialize` | None (one-time) | | |
| `set_max_position_bps` | **Admin** | | Per-investor concentration cap |
| `set_late_penalty_split` | **Admin** | | Treasury vs. investor penalty split |
| `set_marketplace` | **Admin** | | For auto-compound routing |
| `set_auto_compound` | **Investor (Self)** | | Opt-in to reinvestment |
| `release_funds` | **Marketplace Contract** | Caller must be marketplace; invoice fully funded | Transfers pool to SME |
| `record_position` | **Admin** (Marketplace) | Not paused; invoice not frozen; blocks if positions ≥ MAX_POSITIONS_PER_POOL (100) | DoS protection & freeze guard |
| `repay_partial` | **SME (invoice owner)** | Invoice not frozen; no reentrancy lock | |
| `repay` | **SME (invoice owner)** | Invoice not frozen; no reentrancy lock | Full or installment repayment |
| `net_settle` | **SME (payer)** | All invoices belong to payer; ≤ MAX_NETTING_INVOICES (10) | Cross-invoice netting |
| `mark_default` | **Admin** | Grace period elapsed | Manual default declaration |
| `propose_repayment` | **SME (invoice owner)** | High-value invoice requiring approval | Multi-approver workflow |
| `approve_repayment` | **Approver** | Caller in approver list | |
| `execute_repayment` | **Anyone** | Threshold approvals met | Permissionless execution after approval |
| `propose_early_settlement` | **SME (invoice owner)** | Pool open; escrows buyout amount | |
| `accept_early_settlement` | **Investor (position holder)** | Pending offer exists | Unanimous acceptance required |
| `cancel_early_settlement` | **SME (proposer)** | Offer not yet fully accepted | Returns escrowed funds |
| `set_installment_schedule` | **Admin** | No repayment started yet | |
| `split_position` | **Investor (position owner)** | | Fractional position trading |
| `transfer_share` | **Investor (share owner)** | | OTC transfer |
| `list_share_for_sale` | **Investor (share owner)** | | Secondary market listing |
| `buy_share` | **Buyer (Self)** | Share listed for sale | |
| `list_position_for_sale` | **Investor (position owner)** | | Legacy secondary market |
| `buy_position` | **Buyer (Self)** | Position listed for sale | |
| `get_pool` | Permissionless | Read-only | |
| `get_positions` | Permissionless | Read-only | |

**Distribute Yield (Internal):**
- Private function called after repayment/settlement
- Iterates over positions (bounded by MAX_POSITIONS_PER_POOL)
- Auto-compound investors: attempts marketplace listing; falls back to wallet transfer

---

### 6. price_oracle

| Entrypoint | Required Role | Conditional Authorization | Notes |
|------------|---------------|---------------------------|-------|
| `initialize` | None (one-time) | | |
| `set_access_control` | **Admin** | | Post-deployment wiring |
| `add_feeder` | **Admin** | | Feeders bounded by operational policy (~20 max) |
| `remove_feeder` | **Admin** | | |
| `set_price` | **Feeder** | Not paused; reciprocal/peg/rate-change checks | Fresh prices aggregated |
| `set_max_deviation` | **Admin** | | Deprecated; use peg_config or max_rate_change |
| `set_base_currency` | **Admin** | | For multi-hop triangulation |
| `set_peg_config` | **Admin** | | Stablecoin deviation tolerance |
| `remove_peg_config` | **Admin** | | |
| `clear_peg_flag` | **Admin** | | Resume submissions after deviation investigation |
| `set_max_rate_change` | **Admin** | | Per-pair single-update jump limit (NEW) |
| `get_price` | Permissionless | Read-only | Median of fresh feeders |
| `get_price_at` | Permissionless | Read-only | Historical snapshot |
| `get_peg_config` | Permissionless | Read-only | |
| `get_max_rate_change` | Permissionless | Read-only | |
| `is_peg_flagged` | Permissionless | Read-only | |

**Defense Layers (non-authorization):**
- Staleness guard: prices >MAX_STALENESS_SECS excluded
- Reciprocal check: forward/reverse pair consistency (1% tolerance)
- Peg deviation: stablecoin auto-flag on excessive drift
- Rate-change bound: max percentage jump per update (configurable)

---

### 7. dispute_resolution

| Entrypoint | Required Role | Conditional Authorization | Notes |
|------------|---------------|---------------------------|-------|
| `initialize` | None (one-time) | | |
| `open_dispute` | **Challenger (Anyone)** | Invoice defaulted; no existing dispute; within window | Permissionless challenge |
| `submit_evidence` | **Challenger (Self)** | Must be original dispute opener | |
| `resolve_dispute` | **Governance** | Dispute open and unresolved | Multi-sig or DAO |
| `get_dispute` | Permissionless | Read-only | |
| `has_open_dispute` | Permissionless | Read-only | |

**Governance Resolution:**
- `resolver` must be address configured as governance in contract initialization
- Upheld dispute may trigger verifier slashing (cross-contract call to risk_registry)

---

### 8. treasury

| Entrypoint | Required Role | Conditional Authorization | Notes |
|------------|---------------|---------------------------|-------|
| `initialize` | None (one-time) | | |
| `set_allocation` | **Admin** | | Defines revenue distribution percentages |
| `deposit` | **FinancingPool or Marketplace** | Caller must be whitelisted contract | Protocol revenue ingress |
| `withdraw` | **Admin** | | Manual withdrawal to allocation recipients |
| `distribute` | **Anyone** | Balance sufficient for all allocations | Permissionless distribution |
| `get_balance` | Permissionless | Read-only | |
| `get_allocation` | Permissionless | Read-only | |

---

## Conditional Authorization Patterns

### 1. Protocol Pause

**Affects:**
- `marketplace.fund_invoice` → BLOCKED
- `marketplace.list_invoice` → BLOCKED
- `financing_pool.repay` → BLOCKED
- `financing_pool.record_position` → BLOCKED
- `price_oracle.set_price` → BLOCKED

**Implementation:**
```rust
fn require_not_paused(env: &Env) -> Result<(), Error> {
    let ac: Address = env.storage().instance().get(&DataKey::AccessControl)?;
    let ac_client = AccessControlClient::new(env, &ac);
    if ac_client.is_protocol_paused() {
        return Err(Error::ProtocolPaused);
    }
    Ok(())
}
```

### 2. Invoice State Guards

**Transitions:**
- `Created` → `Funded`: only `marketplace.fund_invoice` (when fully funded)
- `Funded` → `Repaid`: only `financing_pool.repay*` or `net_settle`
- Any → `Defaulted`: only `financing_pool.mark_default` (admin) or `admin.set_defaulted`

**Enforcement:** `invoice_nft` state machine validates caller address matches authorized contract for each transition.

### 3. Multisig Quorum

**Applies to:**
- All `access_control` admin operations when `multisig_threshold > 1`
- `marketplace.set_fee_bps` and other config changes
- `financing_pool` admin operations

**Implementation:** Admin must call `access_control.approve_pending_action` to collect signatures; final caller submits with quorum attached.

### 4. Rate Limiting

**Applies to:**
- `invoice_nft.mint_invoice*`: configurable max invocations per time window per SME
- Enforced via `MintRateLimit` storage tracking `(sme, window_start, count)`

---

## Automated Verification Checker

### Tool: `contracts/xtask/src/bin/check_access_matrix.rs`

**Purpose:** Static analysis tool that parses contract source and verifies every `pub fn` has authorization matching this matrix.

**Detection Rules:**

1. **Admin-Only Functions:**
   - Must call `require_admin(&env, &caller)` OR `admin.require_auth()` + `Self::require_admin`
   - Verify multisig quorum check if configured

2. **Role-Specific Functions:**
   - Verifier: must call `require_verifier` or check `is_verifier`
   - Feeder: must call `require_feeder`
   - SME/Investor: must call `caller.require_auth()` + ownership check

3. **Contract-to-Contract:**
   - Must validate `env.current_contract_address() == expected_caller`
   - Or check against storage-configured authorized contract address

4. **Permissionless (Read-Only):**
   - No `require_auth()` calls
   - No state mutations (returns `Result<T, E>` with no writes)

**Checker Algorithm:**
```rust
fn verify_entrypoint(func: &syn::ItemFn, contract: &str) -> Result<(), Drift> {
    let expected_auth = ACCESS_MATRIX.get(contract, func.sig.ident)?;
    let actual_auth = parse_authorization_calls(func)?;
    
    if actual_auth != expected_auth {
        return Err(Drift {
            contract,
            function: func.sig.ident,
            expected: expected_auth,
            actual: actual_auth,
        });
    }
    Ok(())
}
```

**CI Integration:**
```bash
# Run in GitHub Actions on every PR
cargo xtask check-access-matrix
# Exit code 1 if any drift detected
```

**Drift Handling:**
- Fail CI with error message: "Authorization drift detected in {contract}::{function}. Expected {role}, found {actual}. Update docs/ACCESS_MATRIX.md or fix code."
- Prevents accidental privilege escalation or forgotten authorization checks

---

## New Entrypoint Checklist

When adding a new public function to any contract:

1. ☑️ Add entry to this ACCESS_MATRIX.md with required role/authorization
2. ☑️ Implement authorization check matching the matrix entry
3. ☑️ Add test coverage for:
   - ✅ Authorized caller succeeds
   - ✅ Unauthorized caller rejected
   - ✅ Edge case: partial authorization (e.g., wrong SME calling on different invoice)
4. ☑️ Run `cargo xtask check-access-matrix` locally before commit
5. ☑️ Document any conditional authorization (pause, state, multisig)

**Undocumented New Entrypoint = CI Failure**

The automated checker flags any `pub fn` not present in this matrix as an error, forcing developers to consciously decide and document authorization requirements.

---

## Security Audit Trail

**Last Full Audit:** 2026-09-28 (this wave)

**Changes Since Last Audit:**
- Added `MAX_POSITIONS_PER_POOL` bound in `financing_pool.record_position`
- Added `max_rate_change` guard in `price_oracle.set_price`
- Added `MAX_NETTING_INVOICES` bound in `financing_pool.net_settle`
- Added `MAX_VERIFIERS_PER_DEBTOR` bound in `risk_registry.set_debtor_score`

**Known Authorization Gaps:** None as of 2026-09-28.

---

## Appendix: Authorization by Severity

### Critical (Admin-Only, Irreversible)

- `access_control.set_admin`
- `access_control.set_protocol_paused`
- `marketplace.execute_token_whitelist` (after timelock)
- `invoice_nft.set_defaulted`
- `financing_pool.mark_default`

### High (Admin Config, Reversible)

- `marketplace.set_fee_bps`
- `price_oracle.add_feeder` / `remove_feeder`
- `risk_registry.register_verifier` / `remove_verifier`
- `financing_pool.set_max_position_bps`

### Medium (User-Owned Resources)

- `invoice_nft.mint_invoice` (SME)
- `marketplace.list_invoice` (SME)
- `financing_pool.repay` (SME)
- `marketplace.fund_invoice` (Investor)

### Low (Permissionless Reads)

- All `get_*` / `is_*` view functions
- `marketplace.claim_refund` (conditional but permissionless)

---

## References

- Multisig implementation: `contracts/access_control/src/lib.rs`
- Audit trail: `contracts/shared/src/audit.rs`
- Role-based testing: `contracts/tests/access_control_edge_cases.rs`
- Upgrade timelock: `UPGRADE_TIMELOCK_DELAY` constant (86,400 seconds = 24 hours)
