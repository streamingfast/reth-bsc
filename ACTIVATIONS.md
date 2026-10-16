# Network activations

Every hardfork reth-bsc defines, its status on the clusters we serve, and what it changes in the
Firehose block. Statuses come from `src/hardforks/bsc.rs` and were cross-checked against
bnb-chain/bsc `params/config.go` at the same release.

Last checked: 2026-10-16, reth-bsc `v0.1.3` / bsc `v1.8.1`.

## Jenner (BEP-718)

| Cluster | Status |
|---|---|
| mainnet | not scheduled |
| chapel | timestamp `1793241000` (2026-10-29 02:30:00 UTC) |

| Change | Effect on the Firehose block |
|---|---|
| BEP-706 millisecond timestamp precompile at `0x70` | Read-only precompile, nothing new in the block beyond the call itself. |
| BEP-703 Payment Lane | `PaymentLaneContract` (`0x…2007`) is installed by the Jenner system-contract upgrade at block end: one code change in `block.code_changes`, same as geth. Lane metadata getters run on the traced EVM with tracing suspended; geth reads them from raw state. Lane rules decide which transactions a block may hold but add no field. |
| BEP-702 CAS20 registry sentinels | Marker code is planted on the activation and policy registries at block begin of the transition block. Not traced: geth plants them through the raw state at block begin, so its block has no code change for them either. |
| BEP-702 CAS20 stateful precompiles | Tokens only exist after governance opens creation. Each storage write, code install and log a CAS20 call makes is reported to `reth_firehose::precompile_effects` as it happens, so the call carries them in write order, with intermediate values, and keeps them when it reverts, as geth's hooked state does. |
| `0x68` double-sign evidence precompile priced per input byte | Gas only. |

## Earlier forks

| Fork | mainnet | chapel | Effect on the Firehose block |
|---|---|---|---|
| Pasteur | `1787625000` (2026-08-25), active | `1784601000` (2026-07-21), active | System-contract upgrade code changes (traced since `v0.1.2-fh3.1`). |
| Osaka, Mendel | `1777343400` (2026-04-28), active | `1774319400` (2026-03-24), active | Not re-reviewed in this bump. |
| Fermi | `1768357800` (2026-01-14), active | `1762741500` (2025-11-10), active | Not re-reviewed in this bump. |
| Maxwell | `1751250600` (2025-06-30), active | `1748243100` (2025-05-26), active | Not re-reviewed in this bump. |
| Lorentz | `1745903100` (2025-04-29), active | `1744097580` (2025-04-08), active | Not re-reviewed in this bump. |
| Prague, Pascal | `1742436600` (2025-03-20), active | `1740452880` (2025-02-25), active | EIP-2935 system call captured; `HISTORY_STORAGE_ADDRESS` deployment at the transition is emitted as a code change, which geth does not emit (raw state at block begin). |
| Kepler … Bohr, Tycho, Cancun | active | active | Not re-reviewed in this bump. |
| Block-numbered forks (Ramanujan … HertzFix) | active | active | Not re-reviewed in this bump. |
