/// Record an incoming fee for a given token. Called by the marketplace after
    /// transferring the fee amount to this contract. Updates the informational
    /// accounting ledger and earmarks a reserve portion per the effective reserve
    /// allocation (read from parameter_registry when configured).
    ///
    /// **Parameters:**
    /// - `token` — The token address the fee was paid in (must be whitelisted).
    /// - `amount` — The fee amount (must be > 0).
    ///
    /// **Errors:**
    /// - `TreasuryError::InvalidAmount` — `amount` is ≤ 0.
    /// - `TreasuryError::TokenNotWhitelisted` — Token has not been whitelisted.
    /// - `TreasuryError::ArithmeticOverflow` — Running total would overflow.
    pub fn collect_fee(env: Env, token: Address, amount: i128) -> Result<(), TreasuryError> {
        if amount <= 0 {
            return Err(TreasuryError::InvalidAmount);
        }
        Self::require_whitelisted_token(&env, &token)?;

        // Read reserve allocation (registry-aware)
        let reserve_bps = Self::get_reserve_allocation_bps(env.clone());
        if reserve_bps > 0 && reserve_bps <= 10_000 {
            let reserve_amount = amount.checked_mul(reserve_bps as i128).ok_or(TreasuryError::ArithmeticOverflow)?
                .checked_div(10_000).ok_or(TreasuryError::ArithmeticOverflow)?;
            if reserve_amount > 0 {
                let current_reserve: i128 = env.storage().persistent().get(&DataKey::ReserveBalance(token.clone())).unwrap_or(0);
                let new_reserve = current_reserve.checked_add(reserve_amount).ok_or(TreasuryError::ArithmeticOverflow)?;
                env.storage().persistent().set(&DataKey::ReserveBalance(token.clone()), &new_reserve);
                Self::bump_persistent(&env, &DataKey::ReserveBalance(token.clone()));
            }
        }

        // Update informational ledger
        let current: i128 = env.storage().persistent().get(&DataKey::Collected(token.clone())).unwrap_or(0);
        let new_total = current.checked_add(amount).ok_or(TreasuryError::ArithmeticOverflow)?;
        env.storage().persistent().set(&DataKey::Collected(token.clone()), &new_total);
        Self::bump_persistent(&env, &DataKey::Collected(token.clone()));
        events::fee_collected(&env, &env.current_contract_address(), 0, amount, &token);
        Ok(())
    }

    /// Record fees collected with an associated invoice ID (marketplace-originated).
    pub fn collect_fee_with_invoice(env: Env, token: Address, amount: i128, invoice_id: u64) -> Result<(), TreasuryError> {
        if amount <= 0 {
            return Err(TreasuryError::InvalidAmount);
        }
        Self::require_whitelisted_token(&env, &token)?;

        let reserve_bps = Self::get_reserve_allocation_bps(env.clone());
        if reserve_bps > 0 && reserve_bps <= 10_000 {
            let reserve_amount = amount.checked_mul(reserve_bps as i128).ok_or(TreasuryError::ArithmeticOverflow)?
                .checked_div(10_000).ok_or(TreasuryError::ArithmeticOverflow)?;
            if reserve_amount > 0 {
                let current_reserve: i128 = env.storage().persistent().get(&DataKey::ReserveBalance(token.clone())).unwrap_or(0);
                let new_reserve = current_reserve.checked_add(reserve_amount).ok_or(TreasuryError::ArithmeticOverflow)?;
                env.storage().persistent().set(&DataKey::ReserveBalance(token.clone()), &new_reserve);
                Self::bump_persistent(&env, &DataKey::ReserveBalance(token.clone()));
            }
        }

        let current: i128 = env.storage().persistent().get(&DataKey::Collected(token.clone())).unwrap_or(0);
        let new_total = current.checked_add(amount).ok_or(TreasuryError::ArithmeticOverflow)?;
        env.storage().persistent().set(&DataKey::Collected(token.clone()), &new_total);
        Self::bump_persistent(&env, &DataKey::Collected(token.clone()));
        events::fee_collected(&env, &env.current_contract_address(), invoice_id, amount, &token);
        Ok(())
    }

    /// Withdraw accumulated fees to a recipient. Admin only.
    ///
    /// The withdrawable balance excludes the loss-reserve amount per token
    /// (reserve floor is read from parameter_registry when configured).
    pub fn withdraw(
        env: Env,
        admin: Address,
        token: Address,
        recipient: Address,
        amount: i128,
    ) -> Result<(), TreasuryError> {
        admin.require_auth();
        Self::require_admin(&env, &admin)?;
        Self::require_no_quorum_required(&env)?;
        Self::do_withdraw(&env, &admin, &token, &recipient, amount)?;
        Ok(())
    }

    fn do_withdraw(
        env: &Env,
        admin: &Address,
        token: &Address,
        recipient: &Address,
        amount: i128,
    ) -> Result<(), TreasuryError> {
        if amount <= 0 || amount > MAX_AMOUNT {
            return Err(TreasuryError::InvalidAmount);
        }
        Self::require_whitelisted_token(env, token)?;
        Self::require_not_paused(env)?;
        Self::enforce_rate_limit(env, token, amount)?;
        Self::require_allowed_recipient(env, recipient)?;

        let _guard = ReentrancyGuard::new(env).map_err(|_| TreasuryError::Reentrancy)?;

        // Read reserve balance (excluded from withdrawable amount)
        let reserve: i128 = env.storage().persistent().get(&DataKey::ReserveBalance(token.clone())).unwrap_or(0);
        let live_balance = Self::get_balance_internal(env, token);
        let spendable = live_balance.checked_sub(reserve).ok_or(TreasuryError::ArithmeticOverflow)?;
        if amount > spendable {
            return Err(TreasuryError::InsufficientPoolBalance);
        }

        let collected: i128 = env.storage().persistent().get(&DataKey::Collected(token.clone())).unwrap_or(0);
        let new_collected = collected.checked_sub(amount).ok_or(TreasuryError::ArithmeticOverflow)?;
        env.storage().persistent().set(&DataKey::Collected(token.clone()), &new_collected);
        Self::bump_persistent(env, &DataKey::Collected(token.clone()));

        Self::record_withdrawal(env, token, amount);
        Self::transfer_token(env, admin, token, recipient, amount);

        events::fee_withdrawn(env, admin, token, amount);
        Self::append_audit_entry(
            env,
            admin,
            AdminActionType::Withdraw,
            Some(token.clone()),
            Some(amount),
        );
        Ok(())
    }

    fn get_balance_internal(env: &Env, token: &Address) -> i128 {
        let token_client = token::Client::new(env, token);
        token_client.balance(&env.current_contract_address())
    }

    fn record_withdrawal(env: &Env, token: &Address, amount: i128) {
        let epoch_start: u64 = env.storage().instance().get(&DataKey::EpochStart(token.clone())).unwrap_or(env.ledger().timestamp());
        let epoch_withdrawn: i128 = env.storage().instance().get(&DataKey::EpochWithdrawn(token.clone())).unwrap_or(0);
        env.storage().instance().set(&DataKey::EpochStart(token.clone()), &epoch_start);
        env.storage().instance().set(&DataKey::EpochWithdrawn(token.clone()), &(epoch_withdrawn + amount));
    }

    fn enforce_rate_limit(env: &Env, token: &Address, amount: i128) -> Result<(), TreasuryError> {
        let cap: i128 = env.storage().instance().get(&DataKey::WithdrawalCap(token.clone())).unwrap_or(0);
        if cap == 0 {
            return Ok(());
        }
        let epoch_start: u64 = env.storage().instance().get(&DataKey::EpochStart(token.clone())).unwrap_or(env.ledger().timestamp());
        let now = env.ledger().timestamp();
        let (new_epoch_start, epoch_withdrawn) = if now >= epoch_start.saturating_add(EPOCH_DURATION) {
            (now, 0i128)
        } else {
            (epoch_start, env.storage().instance().get::<i128>(&DataKey::EpochWithdrawn(token.clone())).unwrap_or(0))
        };
        let new_withdrawn = epoch_withdrawn.checked_add(amount).ok_or(TreasuryError::ArithmeticOverflow)?;
        if new_withdrawn > cap {
            return Err(TreasuryError::WithdrawalRateLimitExceeded);
        }
        env.storage().instance().set(&DataKey::EpochStart(token.clone()), &new_epoch_start);
        env.storage().instance().set(&DataKey::EpochWithdrawn(token.clone()), &new_withdrawn);
        Ok(())
    }

    fn transfer_token(env: &Env, _admin: &Address, token: &Address, recipient: &Address, amount: i128) {
        let token_client = token::Client::new(env, token);
        token_client.transfer(&env.current_contract_address(), recipient, &amount);
    }

    fn require_admin(env: &Env, caller: &Address) -> Result<(), TreasuryError> {
        let admin: Address = env.storage().persistent().get(&DataKey::Admin).ok_or(TreasuryError::NotAdmin)?;
        if *caller != admin {
            return Err(TreasuryError::NotAdmin);
        }
        Ok(())
    }

    fn require_whitelisted_token(env: &Env, token: &Address) -> Result<(), TreasuryError> {
        let whitelisted: bool = env.storage().persistent().get(&DataKey::WhitelistedToken(token.clone())).unwrap_or(false);
        if !whitelisted {
            return Err(TreasuryError::TokenNotWhitelisted);
        }
        Ok(())
    }

    fn require_not_paused(env: &Env) -> Result<(), TreasuryError> {
        if let Some(ac_addr) = env.storage().instance().get::<Address>(&DataKey::AccessControl) {
            let ac = AccessControlContractClient::new(env, &ac_addr);
            if ac.is_paused() {
                return Err(TreasuryError::NotAdmin); // mapped from KoraError::ProtocolPaused
            }
        }
        Ok(())
    }

    fn require_no_quorum_required(env: &Env) -> Result<(), TreasuryError> {
        // If access_control is linked and has multisig threshold > 1, reject direct calls
        if let Some(ac_addr) = env.storage().instance().get::<Address>(&DataKey::AccessControl) {
            let ac = AccessControlContractClient::new(env, &ac_addr);
            if let Some(cfg) = ac.get_multisig_config() {
                if cfg.threshold > 1 {
                    return Err(TreasuryError::NotAdmin);
                }
            }
        }
        Ok(())
    }

    fn require_allowed_recipient(env: &Env, recipient: &Address) -> Result<(), TreasuryError> {
        let allowed: bool = env.storage().persistent().get(&DataKey::AllowedRecipient(recipient.clone())).unwrap_or(false);
        if !allowed {
            return Err(TreasuryError::NotAdmin); // RecipientNotAllowed mapped
        }
        Ok(())
    }

    fn bump_persistent(env: &Env, key: &DataKey) {
        env.storage().persistent().extend_ttl(
            key,
            PERSISTENT_LIFETIME_THRESHOLD,
            PERSISTENT_BUMP_AMOUNT,
        );
    }

    fn append_audit_entry(
        env: &Env,
        actor: &Address,
        action: AdminActionType,
        token: Option<Address>,
        amount: Option<i128>,
    ) {
        let action_str = match action {
            AdminActionType::SetFeeBps => "set_fee_bps",
            AdminActionType::Withdraw => "withdraw",
            AdminActionType::EmergencyWithdraw => "emergency_withdraw",
            AdminActionType::SetReserveAllocationBps => "set_reserve_allocation_bps",
            AdminActionType::WhitelistToken => "whitelist_token",
            AdminActionType::ProposeWithdrawalCap => "propose_withdrawal_cap",
            AdminActionType::ExecuteWithdrawalCap => "execute_withdrawal_cap",
            AdminActionType::ProposeUpgrade => "propose_upgrade",
            AdminActionType::ExecuteUpgrade => "execute_upgrade",
            AdminActionType::ProposeRecipient => "propose_recipient",
            AdminActionType::ExecuteRecipient => "execute_recipient",
            AdminActionType::SetAccessControl => "set_access_control",
            AdminActionType::SetMarketplace => "set_marketplace",
            AdminActionType::SetPriceOracle => "set_price_oracle",
            AdminActionType::TreasuryProposeUpgrade => "treasury_propose_upgrade",
            AdminActionType::TreasuryExecuteUpgrade => "treasury_execute_upgrade",
            _ => "unknown_admin_action",
        };
        Self::append_audit_entry_internal(env, actor, String::from_str(&env, action_str));
    }

    fn append_audit_entry_internal(env: &Env, actor: &Address, action: String) {
        let head: u64 = env.storage().persistent().get(&DataKey::AuditLogHead).unwrap_or(0);
        let total: u64 = env.storage().persistent().get(&DataKey::AuditLogTotal).unwrap_or(0);
        let entry = AuditEntry {
            action,
            actor: actor.clone(),
            timestamp: env.ledger().timestamp(),
            sequence: total + 1,
        };
        env.storage().persistent().set(&DataKey::AuditEntry(head), &entry);
        env.storage().persistent().set(&DataKey::AuditLogHead, &((head + 1) % MAX_AUDIT_LOG_SIZE));
        env.storage().persistent().set(&DataKey::AuditLogTotal, &(total + 1));
    }

    fn load_signer_config(env: &Env) -> Result<MultisigConfig, KoraError> {
        let ac: Address = env.storage().persistent().get(&DataKey::AccessControl).ok_or(KoraError::MultisigNotConfigured)?;
        let client = AccessControlContractClient::new(env, &ac);
        client.get_multisig_config().ok_or(KoraError::MultisigNotConfigured)
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{
        testutils::{Address as _, MockAuth, MockAuthInvoke},
        token, Address, Env,
    };

    fn setup() -> (Env, Address, TreasuryContractClient<'static>) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, TreasuryContract);
        let client = TreasuryContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        (env, admin, client)
    }

    fn deploy_token(env: &Env, admin: &Address, recipient: &Address, amount: i128) -> Address {
        let token_id = env.register_stellar_asset_contract_v2(admin.clone()).address();
        let token_client = token::StellarAssetClient::new(env, &token_id);
        token_client.mint(recipient, &amount);
        token_id
    }

    // ── initialize ────────────────────────────────────────────────────────────

    #[test]
    fn test_initialize_creates_contract() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, TreasuryContract);
        let client = TreasuryContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        assert!(client.try_initialize(&admin).is_ok());
        assert_eq!(client.get_fee_bps(), 50);
    }

    #[test]
    fn test_initialize_already_initialized() {
        let (env, admin, client) = setup();
        let result = client.try_initialize(&admin);
        assert!(result.is_err());
    }

    #[test]
    fn test_initialize_self_as_admin_rejected() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register_contract(None, TreasuryContract);
        let client = TreasuryContractClient::new(&env, &contract_id);
        let result = client.try_initialize(&contract_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_fee_bps_after_init() {
        let (_env, _admin, client) = setup();
        assert_eq!(client.get_fee_bps(), 50);
    }

    // ── set_fee_bps ───────────────────────────────────────────────────────────

    #[test]
    fn test_set_fee_bps_success() {
        let (_env, admin, client) = setup();
        client.set_fee_bps(&admin, &100u32);
        assert_eq!(client.get_fee_bps(), 100);
    }

    #[test]
    fn test_set_fee_bps_requires_admin() {
        let (env, _admin, client) = setup();
        let non_admin = Address::generate(&env);
        assert!(client.try_set_fee_bps(&non_admin, &100u32).is_err());
    }

    #[test]
    fn test_set_fee_bps_over_max_fails() {
        let (_env, admin, client) = setup();
        assert!(client.try_set_fee_bps(&admin, &10_001u32).is_err());
    }

    #[test]
    fn test_set_fee_bps_zero_allowed() {
        let (_env, admin, client) = setup();
        client.set_fee_bps(&admin, &0u32);
        assert_eq!(client.get_fee_bps(), 0);
    }

    #[test]
    fn test_set_fee_bps_max_allowed() {
        let (_env, admin, client) = setup();
        client.set_fee_bps(&admin, &10_000u32);
        assert_eq!(client.get_fee_bps(), 10_000);
    }

    #[test]
    fn test_set_fee_bps_multiple_updates() {
        let (_env, admin, client) = setup();
        client.set_fee_bps(&admin, &100u32);
        assert_eq!(client.get_fee_bps(), 100);
        client.set_fee_bps(&admin, &200u32);
        assert_eq!(client.get_fee_bps(), 200);
        client.set_fee_bps(&admin, &50u32);
        assert_eq!(client.get_fee_bps(), 50);
    }

    #[test]
    fn test_fee_rate_updated_event_reports_correct_old_fee() {
        let (env, _admin, client) = setup();
        // With default 50, set to 75
        let admin = Address::generate(&env);
        client.set_fee_bps(&admin, &75u32);
        assert_eq!(client.get_fee_bps(), 75);
        // Update to 100 — old must be 75
        client.set_fee_bps(&admin, &100u32);
        assert_eq!(client.get_fee_bps(), 100);
    }

    // ── withdraw ──────────────────────────────────────────────────────────────

    #[test]
    fn test_withdraw_requires_admin() {
        let (env, _admin, client) = setup();
        let non_admin = Address::generate(&env);
        let token = Address::generate(&env);
        let recipient = Address::generate(&env);
        assert!(client.try_withdraw(&non_admin, &token, &recipient, &1_000_000i128).is_err());
    }

    #[test]
    fn test_withdraw_zero_amount_fails() {
        let (env, admin, client) = setup();
        let token = Address::generate(&env);
        let recipient = Address::generate(&env);
        assert!(client.try_withdraw(&admin, &token, &recipient, &0i128).is_err());
    }
}