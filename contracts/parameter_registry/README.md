# Parameter Registry

Canonical on-chain values for governable protocol parameters. The crate exports
`ParameterRegistryContractClient`; keys and values use `RegistryParamKey` and
`RegistryParam` from `kora-shared`.

## Initialization and inputs

Call `initialize(admin, access_control: Option<Address>)` once. The optional
address identifies `AccessControlContractClient`, whose configured multisig
signers authorize parameter proposals. `None` enables admin bootstrap mode.
`set_access_control` configures that dependency later. The registry reads no
process environment variables. Off-chain applications should pass the deployed
registry address explicitly; `PARAMETER_REGISTRY_CONTRACT_ID` is a suggested
application setting, not an input consumed by the contract or deployment script.
Do not pass this address as the forum bridge's `GOVERNANCE_CONTRACT_ID`.

`propose_param_update` records the proposer's approval. Other signers use
`approve_param_update`; `execute_param_update` requires the multisig threshold
and shared upgrade timelock delay before the seven-day expiry. Without a
configured multisig the admin is the sole signer. Direct `admin_set_parameter`
writes are blocked when a multisig is configured. `get_parameter`, `get_value`,
and `get_values_batch` provide read access for contracts and off-chain clients.
Persistent values, proposals, and audit entries receive TTL extensions on writes.
Consumers caching a value must refresh it after updates.

## Tests and CI

```sh
cargo +stable test -p kora-parameter-registry
```

The unit suite covers defaults, bounds, admin writes, the admin-only governance
lifecycle, timelock rejection, cancellation, duplicate approval, batch reads,
and refreshed reads. Workspace fmt/clippy/tests and a dedicated package CI job
include the crate (#912). `kora_parameter_registry.wasm` participates in the
WASM size-budget gate even if a deployment is used only by off-chain readers.
Static onboarding review fixed generated-client result handling, missing event
helpers, an owned-key move, and a test helper's invalid client lifetime. No
build, tests, or formatter were run at the user's request. Configured multisig
integration still needs coverage.
