//! A CAS20 call traced through the Firehose inspector reports its writes the way geth's
//! hooked state does: in write order, with intermediate values, and kept on revert.

use super::{a, call_data, encode_create, w, ADMIN, ALICE};
use crate::{
    evm::{
        api::BscEvm,
        precompiles::cas20::{
            activation::{act_slot, SLOT_ADMIN, SLOT_FEATURES},
            marker_bytecode,
            sigs::*,
            storage::{self, mapping_slot, slot_at, SLOT_BALANCES},
            ACTIVATION_REGISTRY_ADDRESS, FACTORY_ADDRESS, POLICY_REGISTRY_ADDRESS, VARIANT_ASSET,
        },
        transaction::BscTxEnv,
    },
    hardforks::bsc::BscHardfork,
};
use alloy_primitives::{Address, Bytes, B256, U256};
use firehose_tracer::{
    pb::sf::ethereum::r#type::v2::{Block, Call},
    types::{BlockData, BlockEvent, LogData, ReceiptData, TxEvent, TxType},
};
use reth_evm::EvmEnv;
use reth_firehose::inspector::FirehoseInspector;
use revm::{
    context::{result::ExecutionResult, BlockEnv, CfgEnv, TxEnv},
    database::InMemoryDB,
    primitives::TxKind,
    state::AccountInfo,
    InspectEvm,
};

const CHAIN_ID: u64 = 714;

#[test]
fn created_token_reports_writes_in_order_with_intermediate_values() {
    let (block, result) = trace_create(
        1,
        &[call_data(SEL_MINT, &[a(ALICE), w(1000)]), call_data(SEL_MINT, &[a(ALICE), w(500)])],
    );
    assert!(result.is_success(), "creation succeeds: {result:?}");
    let call = root_call(&block);
    assert!(!call.state_reverted);

    // Code install first, as geth's factory calls SetCode before any SetState.
    assert_eq!(call.code_changes.len(), 1, "token code installed once");
    let code_ordinal = call.code_changes[0].ordinal;
    assert!(call.storage_changes.iter().all(|c| c.ordinal > code_ordinal));

    // Logs are interleaved with the storage writes around them, not all after them.
    let first_log = call.logs.iter().map(|l| l.ordinal).min().expect("creation logs");
    assert!(
        call.storage_changes.iter().any(|c| c.ordinal > first_log),
        "storage written after the first log must come after it"
    );
    assert_eq!(call.logs.len(), result.logs().len());

    // The two mints write ALICE's balance slot twice; each write reports its own values.
    let balance_slot = B256::from(mapping_slot(slot_at(SLOT_BALANCES), a(ALICE))).to_vec();
    let balance_writes: Vec<_> = call
        .storage_changes
        .iter()
        .filter(|c| c.key == balance_slot)
        .map(|c| (U256::from_be_slice(&c.old_value), U256::from_be_slice(&c.new_value)))
        .collect();
    assert_eq!(
        balance_writes,
        vec![(U256::ZERO, U256::from(1000)), (U256::from(1000), U256::from(1500))]
    );
}

#[test]
fn reverted_creation_keeps_its_writes() {
    let (block, result) = trace_create(
        2,
        &[call_data(SEL_MINT, &[a(ALICE), w(1)]), call_data(SEL_MINT, &[a(Address::ZERO), w(1)])],
    );
    assert!(matches!(result, ExecutionResult::Revert { .. }), "creation reverts: {result:?}");
    let call = root_call(&block);
    assert!(call.state_reverted);
    assert_eq!(call.code_changes.len(), 1, "geth reports the install inside the reverted call");
    assert!(!call.storage_changes.is_empty());
    assert!(!call.logs.is_empty());
}

fn root_call(block: &Block) -> &Call {
    let trx = block.transaction_traces.first().expect("one transaction");
    trx.calls.first().expect("root call")
}

/// Runs one `createCAS20` transaction from ALICE through a Jenner `BscEvm` carrying the
/// Firehose inspector, and returns the decoded FIRE block and the execution result.
fn trace_create(salt: u64, init_calls: &[Vec<u8>]) -> (Block, ExecutionResult) {
    let input = encode_create(VARIANT_ASSET, w(salt), ALICE, init_calls);
    let (mut tracer, buffer) = firehose_tracer::Tracer::with_buffer(
        firehose_tracer::config::Config::default(),
        firehose_tracer::config::ChainConfig {
            chain_id: CHAIN_ID,
            shanghai_time: Some(0),
            cancun_time: Some(0),
            prague_time: Some(0),
            verkle_time: None,
        },
        "reth-bsc-test",
        "0",
    );
    tracer.on_block_start(BlockEvent {
        block: BlockData { number: 1, ..Default::default() },
        finalized: None,
        flash_block: None,
    });
    tracer.on_tx_start(tx_event(Bytes::from(input.clone())), None);

    let result = {
        let mut evm = jenner_evm(FirehoseInspector::new(&mut tracer));
        let tx = TxEnv::builder()
            .caller(ALICE)
            .chain_id(Some(CHAIN_ID))
            .gas_limit(2_000_000)
            .gas_price(0)
            .kind(TxKind::Call(FACTORY_ADDRESS))
            .data(input.into())
            .build()
            .unwrap();
        evm.inspect_one_tx(BscTxEnv::new(tx)).expect("tx executes")
    };

    let mut receipt = ReceiptData::new(
        0,
        result.tx_gas_used(),
        u64::from(result.is_success()),
        result.tx_gas_used(),
    );
    for (index, log) in result.logs().iter().enumerate() {
        receipt.add_log(LogData::new(
            log.address,
            log.topics().to_vec(),
            log.data.data.clone(),
            index as u32,
        ));
    }
    tracer.on_tx_end(Some(&receipt), None);
    tracer.on_block_end(None);
    drop(tracer);

    (decode_fire_block(&buffer.get_bytes()), result)
}

/// A Jenner EVM with the CAS20 registries planted and the token features opened.
fn jenner_evm<I>(inspector: I) -> BscEvm<InMemoryDB, I> {
    let mut cfg = CfgEnv::new_with_spec(BscHardfork::Jenner).with_chain_id(CHAIN_ID);
    cfg.disable_nonce_check = true;
    cfg.disable_balance_check = true;
    let block = BlockEnv { timestamp: U256::from(1_800_000_000u64), ..Default::default() };
    let mut db = InMemoryDB::default();
    db.insert_account_info(
        ALICE,
        AccountInfo { balance: U256::from(u64::MAX), ..Default::default() },
    );
    for reg in [ACTIVATION_REGISTRY_ADDRESS, POLICY_REGISTRY_ADDRESS] {
        db.insert_account_info(reg, AccountInfo::default().with_code(marker_bytecode()));
    }
    db.insert_account_storage(
        ACTIVATION_REGISTRY_ADDRESS,
        act_slot(SLOT_ADMIN),
        U256::from_be_bytes(a(ADMIN).0),
    )
    .unwrap();
    db.insert_account_storage(
        ACTIVATION_REGISTRY_ADDRESS,
        storage::mapping_slot(act_slot(SLOT_FEATURES), FEATURE_ASSET),
        U256::from(1),
    )
    .unwrap();
    BscEvm::new(EvmEnv::new(cfg, block.into()), db, inspector, true, false)
}

fn tx_event(input: Bytes) -> TxEvent {
    TxEvent {
        tx_type: TxType::Legacy,
        hash: B256::repeat_byte(0x11),
        from: ALICE,
        to: Some(FACTORY_ADDRESS),
        input,
        value: U256::ZERO,
        gas: 2_000_000,
        gas_price: U256::ZERO,
        nonce: 0,
        index: 0,
        v: None,
        r: B256::ZERO,
        s: B256::ZERO,
        max_fee_per_gas: None,
        max_priority_fee_per_gas: None,
        access_list: Vec::new(),
        blob_gas_fee_cap: None,
        blob_hashes: Vec::new(),
        set_code_authorizations: Vec::new(),
    }
}

/// Decodes the base64 protobuf payload of the single FIRE BLOCK line.
fn decode_fire_block(raw: &[u8]) -> Block {
    use base64::Engine as _;
    use prost::Message as _;

    let text = std::str::from_utf8(raw).expect("FIRE output is UTF-8");
    let line = text.lines().find(|l| l.starts_with("FIRE BLOCK ")).expect("a FIRE BLOCK line");
    let payload = line.split(' ').next_back().expect("payload token");
    let bytes = base64::engine::general_purpose::STANDARD.decode(payload).expect("base64 payload");
    Block::decode(bytes.as_slice()).expect("protobuf Block")
}
