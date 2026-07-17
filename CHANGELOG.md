# Changelog

All notable changes to the StreamingFast Firehose fork of bnb-chain/reth-bsc are documented here.

This changelog covers Firehose-specific changes only. For upstream changes, see the
[bnb-chain/reth-bsc repository](https://github.com/bnb-chain/reth-bsc).

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

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
