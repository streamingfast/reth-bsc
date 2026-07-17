# Changelog

All notable changes to the StreamingFast Firehose fork of bnb-chain/reth-bsc are documented here.

This changelog covers Firehose-specific changes only. For upstream changes, see the
[bnb-chain/reth-bsc repository](https://github.com/bnb-chain/reth-bsc).

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

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
