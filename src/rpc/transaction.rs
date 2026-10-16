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
