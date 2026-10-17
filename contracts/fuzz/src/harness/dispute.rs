//! Isolated dispute fixture. The resolution and escrow APIs are independent:
//! the companion escrow uses a mock pool, never an invented resolution hook.
use kora_dispute_escrow::{DisputeEscrowContract, DisputeEscrowContractClient};
use kora_dispute_resolution::{DisputeResolutionContract, DisputeResolutionContractClient};
use kora_shared::types::MultisigConfig;
use soroban_sdk::{
    contract, contractimpl, contracttype,
    testutils::{Address as _, EnvTestConfig, Ledger, LedgerInfo},
    token::StellarAssetClient,
    Address, Env, Vec,
};

#[contract]
pub struct FuzzDisputeAccess;

#[contractimpl]
impl FuzzDisputeAccess {
    // Fixture controls, deliberately outside the production authorization model.
    pub fn configure(env: Env, signer: Address, paused: bool, enabled: bool) {
        env.storage().instance().set(&0u32, &signer);
        env.storage().instance().set(&1u32, &paused);
        env.storage().instance().set(&2u32, &enabled);
    }

    pub fn is_paused(env: Env) -> bool {
        env.storage().instance().get(&1u32).unwrap()
    }

    pub fn get_multisig_config(env: Env) -> MultisigConfig {
        let mut signers = Vec::new(&env);
        if env.storage().instance().get::<_, bool>(&2u32).unwrap() {
            signers.push_back(env.storage().instance().get::<_, Address>(&0u32).unwrap());
        }
        MultisigConfig {
            signers,
            threshold: 1,
        }
    }
}

#[contracttype]
#[derive(Clone)]
enum PoolKey {
    Token,
    Payer,
    Investor,
    Escrow,
    Reserved(u64),
    Released(u64),
}

#[contract]
pub struct FuzzDisputePool;

#[contractimpl]
impl FuzzDisputePool {
    pub fn configure(env: Env, token: Address, payer: Address, investor: Address, escrow: Address) {
        env.storage().instance().set(&PoolKey::Token, &token);
        env.storage().instance().set(&PoolKey::Payer, &payer);
        env.storage().instance().set(&PoolKey::Investor, &investor);
        env.storage().instance().set(&PoolKey::Escrow, &escrow);
    }

    pub fn escrow_terms(env: Env, _invoice_id: u64) -> (Address, Address, i128) {
        (
            env.storage().instance().get(&PoolKey::Token).unwrap(),
            env.storage().instance().get(&PoolKey::Payer).unwrap(),
            100,
        )
    }

    pub fn has_position(env: Env, _invoice_id: u64, investor: Address) -> bool {
        investor
            == env
                .storage()
                .instance()
                .get::<_, Address>(&PoolKey::Investor)
                .unwrap()
    }

    pub fn reserve_escrow(env: Env, escrow: Address, invoice_id: u64, amount: i128) {
        escrow.require_auth();
        assert_eq!(Some(escrow), env.storage().instance().get(&PoolKey::Escrow));
        assert!(!env.storage().instance().has(&PoolKey::Reserved(invoice_id)));
        env.storage()
            .instance()
            .set(&PoolKey::Reserved(invoice_id), &amount);
    }

    pub fn settle_escrow(env: Env, escrow: Address, invoice_id: u64, amount: i128, _to_pool: bool) {
        escrow.require_auth();
        assert_eq!(Some(escrow), env.storage().instance().get(&PoolKey::Escrow));
        assert_eq!(
            Some(amount),
            env.storage().instance().get(&PoolKey::Reserved(invoice_id))
        );
        // A second release is a finding even if token balances still look valid.
        assert!(!env.storage().instance().has(&PoolKey::Released(invoice_id)));
        env.storage()
            .instance()
            .set(&PoolKey::Released(invoice_id), &1u32);
    }

    pub fn releases(env: Env, invoice_id: u64) -> u32 {
        env.storage()
            .instance()
            .get(&PoolKey::Released(invoice_id))
            .unwrap_or(0)
    }
}

pub struct DisputeHarness {
    pub env: Env,
    pub resolution: DisputeResolutionContractClient<'static>,
    pub escrow: DisputeEscrowContractClient<'static>,
    pub access: FuzzDisputeAccessClient<'static>,
    pub pool: FuzzDisputePoolClient<'static>,
    pub token: Address,
    /// Admin (not a signer), governance signer/investor, payer, outsider.
    pub actors: [Address; 4],
}

impl DisputeHarness {
    pub fn deploy() -> Self {
        let mut env = Env::default();
        env.set_config(EnvTestConfig {
            capture_snapshot_at_drop: false,
        });
        env.mock_all_auths();
        env.ledger().set(LedgerInfo {
            timestamp: 100,
            protocol_version: 21,
            sequence_number: 1,
            network_id: Default::default(),
            base_reserve: 10,
            min_temp_entry_ttl: 1_000,
            min_persistent_entry_ttl: 1_000,
            max_entry_ttl: 1_000_000,
        });
        let actors: [Address; 4] = core::array::from_fn(|_| Address::generate(&env));
        let access_id = env.register_contract(None, FuzzDisputeAccess);
        let resolution_id = env.register_contract(None, DisputeResolutionContract);
        let escrow_id = env.register_contract(None, DisputeEscrowContract);
        let pool_id = env.register_contract(None, FuzzDisputePool);
        let access = FuzzDisputeAccessClient::new(&env, &access_id);
        let resolution = DisputeResolutionContractClient::new(&env, &resolution_id);
        let escrow = DisputeEscrowContractClient::new(&env, &escrow_id);
        let pool = FuzzDisputePoolClient::new(&env, &pool_id);
        let token = env
            .register_stellar_asset_contract_v2(actors[0].clone())
            .address();
        StellarAssetClient::new(&env, &token).mint(&actors[2], &1_000);
        access.configure(&actors[1], &false, &true);
        resolution.initialize(&actors[0], &access_id);
        pool.configure(&token, &actors[2], &actors[1], &escrow_id);
        escrow.initialize(&actors[0], &pool_id);
        Self {
            env,
            resolution,
            escrow,
            access,
            pool,
            token,
            actors,
        }
    }
}
