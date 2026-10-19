use crate::{evm::transaction::BscTxEnv, node::evm::config::BscEvmConfig};
use alloy_evm::rpc::{EthTxEnvError, TryIntoTxEnv};
use alloy_rpc_types_eth::TransactionRequest;
use reth_evm::EvmEnvFor;
use reth_rpc_convert::transaction::TxEnvConverter;

/// Uses the same fork environment for RPC transaction defaults and execution.
/// A blob request without `maxFeePerBlobGas` must see the blob
/// price of the overridden environment before that default becomes a value in
/// `TxEnv`. Explicit fees, including zero, remain handled by alloy's converter.
#[derive(Debug, Clone)]
pub struct BscTxEnvConverter(pub BscEvmConfig);

impl TxEnvConverter<TransactionRequest, BscEvmConfig> for BscTxEnvConverter {
    type Error = EthTxEnvError;

    fn convert_tx_env(
        &self,
        request: TransactionRequest,
        env: &EvmEnvFor<BscEvmConfig>,
    ) -> Result<BscTxEnv, Self::Error> {
        request.try_into_tx_env(&self.0.with_block_rules(env.clone()))
    }
}

// Firehose: the node runs with the Firehose wrapper as its EVM config, which shares the
// wrapped config's environment types.
impl TxEnvConverter<TransactionRequest, reth_firehose::FirehoseEvmConfig<BscEvmConfig>>
    for BscTxEnvConverter
{
    type Error = EthTxEnvError;

    fn convert_tx_env(
        &self,
        request: TransactionRequest,
        env: &EvmEnvFor<reth_firehose::FirehoseEvmConfig<BscEvmConfig>>,
    ) -> Result<BscTxEnv, Self::Error> {
        request.try_into_tx_env(&self.0.with_block_rules(env.clone()))
    }
}

/// EVM configs the BSC RPC overrides accept: the plain BSC config and its Firehose wrapper.
pub trait AsBscEvmConfig:
    reth_evm::ConfigureEvm<
    BlockExecutorFactory = <BscEvmConfig as reth_evm::ConfigureEvm>::BlockExecutorFactory,
>
{
    /// The underlying BSC EVM config.
    fn bsc_evm_config(&self) -> &BscEvmConfig;
}

impl AsBscEvmConfig for BscEvmConfig {
    fn bsc_evm_config(&self) -> &BscEvmConfig {
        self
    }
}

impl AsBscEvmConfig for reth_firehose::FirehoseEvmConfig<BscEvmConfig> {
    fn bsc_evm_config(&self) -> &BscEvmConfig {
        &self.inner
    }
}

#[cfg(test)]
mod firehose_tests {
    use crate::{
        chainspec::{bsc::bsc_mainnet, BscChainSpec},
        hardforks::bsc::BscHardfork,
        node::evm::config::BscEvmConfig,
    };
    use alloy_evm::Evm as _;
    use alloy_primitives::U256;
    use reth_chainspec::ForkCondition;
    use reth_evm::ConfigureEvm;
    use revm::database::EmptyDB;
    use std::sync::Arc;

    const JENNER_AT: u64 = 1_800_000_000;

    #[test]
    fn firehose_config_applies_block_override_fork_rules() {
        let mut spec = bsc_mainnet();
        spec.hardforks.insert(BscHardfork::Jenner, ForkCondition::Timestamp(JENNER_AT));
        let config = BscEvmConfig::new(Arc::new(BscChainSpec::from(spec)));
        let header = alloy_consensus::Header {
            number: 40_000_000,
            timestamp: JENNER_AT - 100,
            gas_limit: 140_000_000,
            base_fee_per_gas: Some(0),
            difficulty: U256::from(2u64),
            excess_blob_gas: Some(0),
            blob_gas_used: Some(0),
            ..Default::default()
        };
        let mut env = config.evm_env(&header).unwrap();
        assert_ne!(env.cfg_env.spec, BscHardfork::Jenner);
        // An RPC time override that crosses Jenner.
        env.block_env.inner.timestamp = U256::from(JENNER_AT + 100);

        let firehose = reth_firehose::FirehoseEvmConfig::new(config);
        assert_eq!(
            firehose.evm_with_env(EmptyDB::default(), env.clone()).cfg_env().spec,
            BscHardfork::Jenner
        );
        assert_eq!(
            firehose
                .evm_with_env_and_inspector(EmptyDB::default(), env, revm::inspector::NoOpInspector)
                .cfg_env()
                .spec,
            BscHardfork::Jenner
        );
    }
}
