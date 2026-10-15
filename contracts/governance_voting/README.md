# Kora Governance Voting Contract

A comprehensive on-chain governance system for the Kora Protocol, supporting vote delegation and quadratic voting for community-driven decision making.

## Deployment inputs and workspace CI

`GovernanceVotingContractClient` is the generated Rust client for this crate.
Initialize it with the admin address and the access-control contract address.
These are explicit on-chain inputs; the contract reads no environment variables.
For the forum bridge, `GOVERNANCE_CONTRACT_ID` must be this deployment's address,
and `SOROBAN_RPC_URL` selects its network's RPC endpoint. See the bridge's
[configuration and client mapping](../../services/governance-sync/README.md#configuration).
The separate governance upgrade and parameter-registry addresses are incompatible.

CI runs `cargo test -p kora-governance-voting` as a dedicated matrix entry (#912),
with workspace fmt/clippy/tests also including the crate. Persistent proposal,
vote, weight, and delegation writes extend TTL using the contract's bump helper.
Release artifact `kora_governance_voting.wasm` is included in the size gate.
No build, tests, or formatter were run for this onboarding change at the user's
request; coverage percentages below are existing targets, not measured results.

## Features

### 1. Vote Delegation

Stakeholders can delegate their voting weight to trusted representatives without transferring underlying stake ownership.

**Key Properties:**
- **Single-level delegation**: No transitive/chained delegation
- **Revocable at any time**: Delegators maintain full control
- **Ownership retained**: Underlying stake never transfers
- **Direct vote precedence**: Delegator's direct vote overrides delegation

**Prevents:**
- Circular delegation (A→B→A)
- Self-delegation
- Double-counting

**Use Cases:**
- Small stakeholders who lack time/expertise
- Community representatives accumulating voting power
- Flexible governance participation

### 2. Quadratic Voting

Optional voting mode for community sentiment proposals where vote cost scales quadratically.

**How it Works:**
- Voice credits derived from voting weight (stake)
- Votes = √(voice_credits_spent)
- Examples:
  - 1 credit → 1 vote
  - 4 credits → 2 votes
  - 9 credits → 3 votes
  - 25 credits → 5 votes
  - 100 credits → 10 votes

**Proposal Type Restrictions:**
- ✅ **Sentiment proposals**: Community preference, protocol direction
- ❌ **Financial proposals**: Must use standard stake-weighted voting

**Benefits:**
- Better captures broad community preference
- Reduces disproportionate influence of large holders
- Allows expressing intensity of preference

### 3. Proposal Types

- **Financial**: Treasury allocation, grants (standard voting only)
- **Sentiment**: Community preferences, non-financial direction
- **Upgrade**: Protocol upgrades
- **ParameterChange**: Configuration updates

### 4. Vote Tallying

**Standard Mode:**
- Voter's full voting weight applied to their choice
- Simple stake-weighted voting

**Quadratic Mode:**
- Voter specifies voice credits to spend
- Actual votes = integer_sqrt(credits)
- Must not exceed voter's available weight

**Delegation Handling:**
- Delegates receive aggregated weight from all delegators
- Direct votes use voter's own weight only
- No double-counting (voted addresses excluded from delegation tallies)

## Architecture

### Data Structures

```rust
pub enum VotingMode {
    Standard,   // Full weight voting
    Quadratic,  // Quadratic cost voting
}

pub enum ProposalType {
    Financial,
    Sentiment,
    Upgrade,
    ParameterChange,
}

pub struct Proposal {
    pub id: u64,
    pub proposer: Address,
    pub title: String,
    pub description: String,
    pub proposal_type: ProposalType,
    pub voting_mode: VotingMode,
    pub votes_for: i128,
    pub votes_against: i128,
    pub votes_abstain: i128,
    pub created_at: u64,
    pub expires_at: u64,
    pub executed: bool,
    pub quorum_required: i128,
}

pub struct VoteRecord {
    pub voter: Address,
    pub choice: VoteChoice,
    pub weight: i128,
    pub voice_credits_spent: i128,
    pub voted_at: u64,
}
```

### Storage Keys

- `Admin`: Admin address
- `VotingWeight(Address)`: Stake/weight for address
- `Delegation(Address)`: Delegator → delegate mapping
- `DelegatedBy(Address)`: Reverse index for efficient weight calculation
- `Proposal(u64)`: Proposal data by ID
- `VoteRecord(u64, Address)`: Vote records per proposal per voter

## API Reference

### Initialization

```rust
pub fn initialize(
    env: Env,
    admin: Address,
    access_control: Address,
) -> Result<(), GovernanceError>
```

Initialize the contract with admin and access control addresses.

### Weight Management

```rust
pub fn set_voting_weight(
    env: Env,
    admin: Address,
    address: Address,
    weight: i128,
) -> Result<(), GovernanceError>
```

Set voting weight for an address (admin only). In production, this would sync with actual stake.

```rust
pub fn get_voting_weight(env: Env, address: Address) -> i128
```

Get total voting weight (own weight + delegated weight if delegate).

### Delegation

```rust
pub fn delegate(
    env: Env,
    delegator: Address,
    delegate: Address,
) -> Result<(), GovernanceError>
```

Delegate voting weight to another address. Requires `delegator.require_auth()`.

```rust
pub fn revoke_delegation(
    env: Env,
    delegator: Address,
) -> Result<(), GovernanceError>
```

Revoke current delegation. Requires `delegator.require_auth()`.

```rust
pub fn get_delegate(env: Env, delegator: Address) -> Option<Address>
```

Get the current delegate for an address.

### Proposals

```rust
pub fn create_proposal(
    env: Env,
    proposer: Address,
    title: String,
    description: String,
    proposal_type: ProposalType,
    voting_mode: VotingMode,
    quorum_required: i128,
    duration: u64,  // 0 = use default (7 days)
) -> Result<u64, GovernanceError>
```

Create a new proposal. Returns proposal ID.

**Validation:**
- Financial proposals cannot use quadratic voting
- Duration defaults to 7 days if zero

```rust
pub fn get_proposal(env: Env, proposal_id: u64) -> Option<Proposal>
```

Retrieve proposal details.

### Voting

```rust
pub fn vote(
    env: Env,
    voter: Address,
    proposal_id: u64,
    choice: VoteChoice,
    voice_credits: i128,
) -> Result<(), GovernanceError>
```

Cast a vote on a proposal.

**Parameters:**
- `voice_credits`: For quadratic voting (ignored for standard voting)

**Rules:**
- Cannot vote twice on same proposal
- Must have voting weight > 0
- Proposal must not be expired or executed
- For quadratic: credits must be > 0 and ≤ voting weight

```rust
pub fn get_vote_record(
    env: Env,
    proposal_id: u64,
    voter: Address,
) -> Option<VoteRecord>
```

Get vote record for a voter on a proposal.

### Execution

```rust
pub fn execute_proposal(
    env: Env,
    admin: Address,
    proposal_id: u64,
) -> Result<(), GovernanceError>
```

Execute a proposal (admin only). Requires quorum to be met.

## Edge Cases & Constraints

### Delegation

1. **Mid-vote revocation**: Affects only future votes, not past tallies
2. **Large delegate fan-in**: Delegated weight calculation is O(n) where n = number of delegators
   - Acceptable for reasonable delegate sizes (<100 delegators)
   - Gas cost scales linearly with delegator count
3. **Re-delegation**: Old delegate loses weight immediately
4. **Delegation without weight**: Allowed but contributes zero to delegate's weight

### Quadratic Voting

1. **Integer square-root**: Uses binary search algorithm
   - Always returns floor(sqrt(n))
   - No floating-point math, fully deterministic
2. **Sybil resistance limitation**: Splitting stake across addresses can game the system
   - Documented known limitation given on-chain identity constraints
   - Mitigated by proposal type restrictions (quadratic only for sentiment, not financial)
3. **Rounding consistency**: All voters use same integer sqrt implementation
4. **Credit validation**: Strictly enforced ≤ voting weight

### Vote Tallying

1. **Precedence rule**: Direct vote always counts, delegation ignored for that voter
2. **Double-counting prevention**: Voters who cast direct votes excluded from delegation aggregation
3. **Arithmetic safety**: All tallies use checked arithmetic with overflow protection

## Testing

Test coverage: **90%+**

### Delegation Tests

- ✅ Delegation and revocation
- ✅ Circular delegation prevention
- ✅ Direct vote precedence
- ✅ Mid-vote revocation
- ✅ Large delegate fan-in (50+ delegators)
- ✅ Re-delegation
- ✅ Self-delegation rejection

### Quadratic Voting Tests

- ✅ Cost calculation correctness
- ✅ Voice credits exceed weight rejection
- ✅ Zero/negative credits rejection
- ✅ Mode selection per proposal
- ✅ Integer sqrt comprehensive coverage
- ✅ Multiple voters aggregation

### General Tests

- ✅ Proposal creation and validation
- ✅ Vote record tracking
- ✅ Already-voted prevention
- ✅ Quorum enforcement
- ✅ Proposal execution
- ✅ Abstain votes

Run tests:

```bash
cargo test --package kora-governance-voting --lib
```

## Security Considerations

1. **Authorization**: All state-changing functions require `require_auth()`
2. **Overflow protection**: Checked arithmetic throughout
3. **Reentrancy**: Not applicable (no cross-contract calls in vote path)
4. **Delegation circularity**: Prevented at delegation time
5. **Vote finality**: Cannot change vote once cast
6. **Admin privileges**: Limited to weight management and execution

## Integration Example

```rust
use kora_governance_voting::{
    GovernanceVotingContractClient,
    ProposalType,
    VotingMode,
    VoteChoice,
};

// Initialize
let client = GovernanceVotingContractClient::new(&env, &contract_id);
client.initialize(&admin, &access_control);

// Set voting weights (in production, synced from staking contract)
client.set_voting_weight(&admin, &alice, &1000);
client.set_voting_weight(&admin, &bob, &500);

// Alice delegates to Bob
client.delegate(&alice, &bob);

// Bob now has 1500 weight (500 own + 1000 delegated)
let bob_weight = client.get_voting_weight(&bob); // 1500

// Create a sentiment proposal with quadratic voting
let proposal_id = client.create_proposal(
    &proposer,
    &String::from_str(&env, "Protocol Direction"),
    &String::from_str(&env, "Should we..."),
    &ProposalType::Sentiment,
    &VotingMode::Quadratic,
    &100, // quorum
    &0,   // default duration
);

// Bob votes with 49 voice credits → 7 votes (sqrt(49))
client.vote(&bob, &proposal_id, &VoteChoice::For, &49);

// Execute if quorum met
client.execute_proposal(&admin, &proposal_id);
```

## Future Enhancements

- Multi-level delegation (with cycle detection)
- Delegated voting on behalf of delegators
- Time-weighted voting (longer stake lock = more weight)
- Vote history and analytics
- Snapshot-based voting (prevent flash loans)
- Proposal categories with different quorum rules

## License

MIT
