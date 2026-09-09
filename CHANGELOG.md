# Changelog

All notable changes to the StreamingFast Firehose fork of bnb-chain/reth-bsc are documented here.

This changelog covers Firehose-specific changes only. For upstream changes, see the
[bnb-chain/reth-bsc repository](https://github.com/bnb-chain/reth-bsc).

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

## v0.1.1-fh3.2

### Fixed

- Stop advertising a finalized block that is not an ancestor of the block being emitted
  (`streamingfast/reth` `bnb-v0.1.1-fh3.2`). Every `FIRE BLOCK` line carried the node's finalized
  head as of the moment the block executed, so blocks from a side branch were published with a LIB
  number from the canonical chain — downstream then marked its own block at that height
  irreversible and saw it replaced by the reorg. Seen on mainnet at 120653740, where a four-block
  branch was published with LIB 120653741, one of them naming a height above the block itself.

## v0.1.1-fh

Release ready for prime time


## v0.1.1-fh-beta-13

### Fixed

- Include the SELFDESTRUCT refund when resolving an account's post-transaction balance
  (`streamingfast/bnb-reth` port of `streamingfast/reth` `v2.3.0-fh-7`). On the truly-destroyed
  path (EIP-6780: contract created in the same transaction, or pre-Cancun) revm credits the
  beneficiary in place and records the move only inside its `AccountDestroyed` journal entry — no
  `BalanceTransfer` is pushed — so the journal walk backing the `RewardTransactionFee` and
  `GasRefund` events missed it. A coinbase or sender that received a suicide refund then reported
  an `old_balance` contradicting the `SuicideRefund` event emitted moments earlier.

## v0.1.1-fh-beta-12

### Fixed

- Restored two beta-4/6 fixes dropped during the v0.1.1 rebase: the EIP-2935
  history-storage system call is captured again (`transact_system_call` routes through the
  inspector when tracing), and the canonical block size again excludes blob sidecars.
  (beta-10/11 traces were missing `system_calls` entirely and misreported `size` on blob
  blocks.)

## v0.1.1-fh-beta-10

### Changed

- Rebased onto upstream `v0.1.1` (`457f81a`): Pasteur mainnet activation scheduled
  (2026-08-25), upstream network/blocks-by-range improvements, and the new
  `bnb-chain/reth` pin (`c13b0986`) which streams finalization-appended (system-tx)
  receipts to the engine receipt-root task. The Firehose fork of the reth crates moved to
  `streamingfast/bnb-reth` branch `firehose/0.1.x-bsc` accordingly, with the same
  finalization-receipt streaming mirrored in the Firehose-traced engine execution path.
- Carries the beta-8 hard funding gate (replay funding never active under the Firehose
  inspector) and the beta-9 fast RocksDB `TransactionHashNumbers` healing.

Note: the block 106696194 pipeline divergence (deposit system tx short by one tx fee) is
NOT known to be fixed by this rebase — it reproduces with tracing disabled and is being
reported upstream.

## v0.1.0-fh-beta-7

### Added

- `FIREHOSE_DISABLED=true` kill-switch: skips tracer initialization entirely, so the node
  executes through the plain untraced path, byte-identical to un-instrumented reth-bsc. The
  firehose ExEx idles in no-op mode (still advancing `FinishedHeight` so the WAL prunes). Ops
  lever for isolating tracing-induced behavior — e.g. the block 106696194 deposit-value
  divergence under investigation.

## v0.1.0-fh-beta-3

### Fixed

- Panic during mainnet sync ("mismatch between call log and receipt log BlockIndex"): system
  transactions bypass the generic wrapper's per-transaction log accounting, so a log-bearing
  system tx (e.g. the validator deposit) left the block-wide log counter behind and the next
  system tx's call logs lagged its receipt logs. The chain executor now reports each system
  tx's committed log count and the inspector folds it into block-wide log indices.

## v0.1.0-fh-beta-2

### Fixed

- Panic during mainnet pipeline sync ("caller expected to be in transaction state"): BSC's
  internal consensus reads (validator-set / turn-length `eth_call`s) run on the same
  inspector-carrying EVM as real transactions and fired tracer hooks between transactions.
  These reads are now executed with Firehose tracing suspended — matching geth, which never
  traces them.

## v0.1.0-fh-beta-1

### Fixed

Parity with the geth-BSC Firehose reference (`streamingfast/go-ethereum`, `release/bnb-1.x-fh3.0`):

- Transaction fee (and EIP-4844 blob fee) balance changes are now credited to the consensus
  `SYSTEM_ADDRESS` (`0xffff…fffe`) instead of the block beneficiary, matching BSC's
  fee routing (`REASON_REWARD_TRANSACTION_FEE` / `REASON_REWARD_BLOB_FEE`).
- Parlia system transactions (deposit, slash, finality reward, validator-set update, genesis
  and Feynman initialization) are now traced as ordinary transaction traces at their actual
  execution point inside the end-of-block finalize step, instead of empty placeholder traces
  during body iteration with the real EVM work mis-attributed to a system-call window.
- The `distribute_incoming` sweep (SYSTEM_ADDRESS → validator, a direct non-EVM state write)
  now emits its two balance changes (`REASON_REWARD_TRANSACTION_FEE`), like geth's
  `BalanceDecrease/IncreaseBSCDistributeReward` hooks.
- System-contract code upgrades and the Prague history-storage deploy (direct non-EVM code
  installs) now emit code changes.

Known divergences from geth, by design or accepted: no gas-change events (dropped in Firehose
Ethereum tracer v5); system-tx sender nonce changes inside the EVM are emitted (geth suppresses
them under its backward-compatibility flag); the pre-Kepler two-step system-reward split emits a
single sweep pair here (modern blocks are identical).

## v0.1.0-fh-beta

### Added

- Firehose instrumentation for BSC: the `reth` crates are consumed from the
  [streamingfast/bnb-reth](https://github.com/streamingfast/bnb-reth) fork (branch
  `firehose/0.x-bsc`), which carries the `reth-firehose` crate and the engine-tree /
  pipeline tracing hooks. The `reth-bsc` binary initializes the process-wide tracer,
  wraps `BscEvmConfig` in `FirehoseEvmConfig` (pipeline path), and installs the
  Firehose ExEx — every validated block is emitted as a `FIRE BLOCK` line on stdout.
- `Dockerfile.sf` / `sf-release.yml`: Docker image build that bundles `fireeth` driving
  `reth-bsc` as its reader node, published to `ghcr.io/streamingfast/reth-bsc`.

### Fixed

- A fresh node (still at genesis) could never peer: the chain-spec `head()` helpers leave
  the head hash at the zero default, eth/69 status advertised that zero hash, and the
  handshake rejects zero blockhashes. The network head now falls back to the genesis hash.
