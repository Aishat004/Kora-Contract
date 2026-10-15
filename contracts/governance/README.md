# Governance Contract

Stake-weighted upgrade proposals with on-chain or anchored off-chain voting,
plus a configurable fast-track path for registered critical bug reports.

## Typed client and configuration

The crate exports `GovernanceContractClient` from [src/lib.rs](src/lib.rs).
Initialize it with `admin`, `access_control`, `min_stake`, `quorum`,
`threshold_bps`, and `voting_duration` (seconds). The contract reads no process
environment variables; these are explicit on-chain inputs. Pass this deployment's
address explicitly when constructing its typed client. The forum bridge uses
`GOVERNANCE_CONTRACT_ID` for a **governance_voting** deployment; see its
[address mapping](../../services/governance-sync/README.md#configuration).
Do not reuse an address between those two different contract interfaces.

Admin sets voting stake and target hashes. `create_proposal`, `vote`, and
`execute_proposal` form the standard path. `designate_offchain_voting` disables
on-chain voting for a proposal, and `anchor_offchain_result` accepts one admin
tally and summary hash after its voting deadline. Execution records an upgrade
in the shared timelock; the target contract is not upgraded by this call.

Fast track is disabled until `configure_fast_track(admin, enabled,
threshold_bps, timelock_seconds, max_proposal_size)` enables it. Admin registers
a report using `register_critical_bug_report(admin, report_id: u64)`.
`create_fast_track_proposal` takes that report ID and `proposal_size_bytes`;
`vote_fast_track` and `execute_fast_track_proposal` operate on the resulting
proposal. Unset fallbacks are 7,500 basis points, 7,200 seconds, and 1,000 bytes.
`publish_fast_track_disclosure` records a disclosure flag; the current execution
path does not enforce that flag. Both proposal paths schedule shared timelock
upgrades, including fast track. Operators must account for that additional delay.

## Tests and CI

```sh
cargo +stable test -p kora-governance
```

The unit suite covers standard execution/replay, duplicate votes, off-chain
anchoring timing/replay, and the disabled fast-track guard. CI runs this package
separately and includes it in workspace fmt, clippy, and test jobs (#912).
Release artifact `kora_governance.wasm` participates in the WASM size gate.
The onboarding changes were reviewed statically; no build, test, or formatter
was run at the user's request. Full fast-track lifecycle and target-upgrade
integration coverage remain gaps.
