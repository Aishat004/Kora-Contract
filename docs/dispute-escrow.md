# Disputed repayment escrow (#747)

`dispute_escrow` holds a disputed payment in the pool's token until a single
adjudicator decides whether it should repay the invoice or return to its SME
payer. The admin can be a governance multisig contract address. It is separate
from the existing `dispute_resolution` contract for challenges to defaults.

## Deployment

1. Deploy `dispute_escrow`; call `initialize(admin, financing_pool)`.
2. Call `financing_pool.set_dispute_escrow(admin, escrow_address)` once.
3. Make sure the financing pool is authorized to mark the invoice NFT repaid
   and the escrow contract has sufficient ledger lifetime for open disputes.

## Flow

`open_dispute(challenger, payer, invoice_id, amount)` requires signatures from
the challenger and SME payer. The challenger must be the SME or an investor
with a recorded position in that invoice. The amount must be positive and no
greater than the pool's remaining debt, in the pool token's smallest units.
The escrow reserves the invoice in the pool and transfers the payment directly
from the SME into escrow. It does **not** increment `repaid_amount`, protocol
repayment statistics, or investor balances. Normal `repay`, `repay_partial`,
`net_settle`, and `mark_default` are blocked while the reservation is open.

Before seven days have elapsed, the configured admin calls
`resolve_dispute(resolver, invoice_id, pay_pool)`. `pay_pool=true` transfers
the payment to the financing pool and calls its authenticated
`settle_escrow` hook; that hook credits repayment, distributes yield and marks
the NFT repaid if the invoice is fully settled. `pay_pool=false` returns the
token to the SME payer and clears the reservation without crediting repayment.
After seven days, anyone can call `resolve_timeout(invoice_id)` to refund the
SME and unblock normal settlement. An invoice may enter this escrow **once**:
neither resolution nor timeout permits a new dispute for the same invoice.

The escrow and pool callbacks execute in one transaction. A failed transfer
or settlement rolls back the entire resolution; the dispute remains open.
Inspect `get_dispute(invoice_id)` and the pool's `get_pool(invoice_id)` to
verify the outcome. The existing pool's late-penalty and installment rules
should be considered before choosing an escrow amount; this path applies the
exact deposited pool-token amount and does not advance installment schedules.

## Validation

### Workspace and release size gate (#911)

`kora-dispute-escrow` is a workspace member. `make build` explicitly builds
the deployable contract packages, including this crate, and produces
`target/wasm32-unknown-unknown/release/kora_dispute_escrow.wasm`.
`make build-optimized` optimizes it alongside the other deployable contracts.
The WASM size workflow requires the release artifact and applies the
`dispute_escrow` budget in `wasm-budgets.json` (122,880 bytes); an overage
fails CI under the existing size-increase approval policy. Release hash
recording includes the same artifact. See [performance budgets](PERFORMANCE.md)
for the provisional-cap rationale; this onboarding did not run builds or
measure the binary.

`scripts/deploy.sh` uses the exact `kora_dispute_escrow.wasm` filename when
`DEPLOY_DISPUTE_ESCROW=true` is set (default: false). This is opt-in because
the current financing pool does not implement the required `escrow_terms`,
`reserve_escrow`, `settle_escrow`, and `set_dispute_escrow` hooks. Enabling it
deploys and initializes the escrow and records its address/hash, but pool
integration must be implemented before using the custody flow above.

### State-machine fuzzing (#919)

See [the dispute fuzz model and coverage gaps](dispute-fuzz.md) and the
[fuzz target inventory](../contracts/fuzz/README.md). Resolution of defaults
and repayment custody are separate APIs; a mock cannot establish production
pool/token integration safety.

The escrow crate has tests for custody before resolution, both adjudication
routes, repeat-dispute rejection, and the timeout boundary. Run
`cargo test -p kora-dispute-escrow` and the financing pool suite in a working
Rust workspace, then measure coverage against the repository's 90% gate.
