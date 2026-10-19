use crate::{
    evm::{
        api::{BscContext, BscEvm},
        block_env::BscBlockEnv,
        transaction::BscTxEnv,
    },
    hardforks::bsc::BscHardfork,
};
use reth_evm::{precompiles::PrecompilesMap, Database, EvmEnv, EvmFactory};
use revm::context::result::{EVMError, HaltReason};
use revm::inspector::NoOpInspector;
use reth_revm::Inspector;

/// Factory producing [`BscEvm`].
#[derive(Debug, Default, Clone, Copy)]
#[non_exhaustive]
pub struct BscEvmFactory;

impl EvmFactory for BscEvmFactory {
    type Evm<DB: Database, I: Inspector<BscContext<DB>>> = BscEvm<DB, I>;
    type Context<DB: Database> = BscContext<DB>;
    type Tx = BscTxEnv;
    type Error<DBError: core::error::Error + Send + Sync + 'static> = EVMError<DBError>;
    type HaltReason = HaltReason;
    type Spec = BscHardfork;
    type BlockEnv = BscBlockEnv;
    type Precompiles = PrecompilesMap;

    fn create_evm<DB: Database>(
        &self,
        db: DB,
        input: EvmEnv<BscHardfork, BscBlockEnv>,
    ) -> Self::Evm<DB, NoOpInspector> {
        // Check if we're in a trace/debug context by examining the database type
        // CacheDB is used in trace scenarios where we need to replay transactions
        let type_name = std::any::type_name::<DB>();
        let is_trace = type_name.contains("CacheDB");
        
        BscEvm::new(input, db, NoOpInspector {}, false, is_trace)
    }

    fn create_evm_with_inspector<DB: Database, I: Inspector<Self::Context<DB>>>(
        &self,
        db: DB,
        input: EvmEnv<BscHardfork, BscBlockEnv>,
        inspector: I,
    ) -> Self::Evm<DB, I> {
        // `trace` enables `fund_beneficiary_for_system_tx_replay`, a stand-in for
        // `distribute_incoming`'s validator credit that is only correct when replaying a single
        // transaction against archive state (RPC debug_trace, which wraps the DB in CacheDB).
        // Full-block execution — including the Firehose inspector path — runs `distribute_incoming`
        // itself, so funding again double-credits the validator and shows up as a phantom GAS_BUY
        // on the validator in the Firehose trace. Gate on the same CacheDB heuristic as `create_evm`
        // so only the single-tx replay path funds.
        // Hard gate: never fund under Firehose full-block execution, regardless of what the DB
        // type-name heuristic concludes — the heuristic is inherently fragile (it inspects
        // monomorphized type names), and a misfire double-credits the validator with committed
        // state, i.e. a consensus break. The Firehose inspector only ever drives full blocks.
        let is_firehose = std::any::type_name::<I>().contains("FirehoseInspector");
        let is_trace = !is_firehose && std::any::type_name::<DB>().contains("CacheDB");
        BscEvm::new(input, db, inspector, true, is_trace)
    }
}
