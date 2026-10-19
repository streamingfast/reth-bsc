//! A CAS20 call traced through the Firehose inspector reports its writes the way geth's
//! hooked state does: in write order, with intermediate values, and kept on revert.

use super::{a, call_data, derive_address, encode_create, w, ADMIN, ALICE, BOB};
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
    inspector::InspectorEvmTr,
    primitives::TxKind,
    state::AccountInfo,
    InspectCommitEvm,
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

#[test]
fn a_write_whose_gas_is_not_paid_is_not_reported() {
    let create =
        encode_create(VARIANT_ASSET, w(3), ALICE, &[call_data(SEL_MINT, &[a(ALICE), w(1000)])]);
    let token = derive_address(VARIANT_ASSET, ALICE, w(3));
    let transfer = call_data(SEL_TRANSFER, &[a(BOB), w(400)]);
    let bob_slot = B256::from(mapping_slot(slot_at(SLOT_BALANCES), a(BOB))).to_vec();

    // Some(bob's balance written) for the transfer at `gas`, None when it succeeds.
    let run = |gas: u64| {
        let (block, result) =
            trace_tx(&[(FACTORY_ADDRESS, create.clone(), 2_000_000)], token, transfer.clone(), gas);
        if result.is_success() {
            return None;
        }
        let call = root_call(&block);
        Some(call.storage_changes.iter().any(|c| c.key == bob_slot))
    };

    let (mut lo, mut hi) = (21_000u64, 200_000u64);
    assert!(run(hi).is_none(), "the transfer succeeds with ample gas");
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if run(mid).is_none() {
            hi = mid
        } else {
            lo = mid
        }
    }
    let enough = hi;

    // Bob's slot goes from zero to non-zero (20,000 gas) and only the Transfer log is charged
    // after it, so a reported write leaves the transfer short by less than the write costs.
    // Reporting it once the 2,300-gas sentry passes would show it far below that.
    let mut reported = 0;
    let mut unreported = 0;
    for gas in (enough - 30_000..enough).step_by(250) {
        match run(gas) {
            Some(true) => {
                assert!(enough - gas < 5_000, "write reported {} gas short", enough - gas);
                reported += 1;
            }
            Some(false) => unreported += 1,
            None => panic!("transfer succeeded below the minimum gas"),
        }
    }
    assert!(reported > 0 && unreported > 0, "the sweep crosses the write");
}

fn root_call(block: &Block) -> &Call {
    let trx = block.transaction_traces.first().expect("one transaction");
    trx.calls.first().expect("root call")
}

/// Runs one `createCAS20` transaction from ALICE through a Jenner `BscEvm` carrying the
/// Firehose inspector, and returns the decoded FIRE block and the execution result.
fn trace_create(salt: u64, init_calls: &[Vec<u8>]) -> (Block, ExecutionResult) {
    let input = encode_create(VARIANT_ASSET, w(salt), ALICE, init_calls);
    trace_tx(&[], FACTORY_ADDRESS, input, 2_000_000)
}

/// Runs the `setup` transactions `(to, input, gas limit)` untraced, then one traced transaction,
/// all from ALICE in one block through a Jenner `BscEvm` carrying the Firehose inspector, and
/// returns the decoded FIRE block and the traced transaction's result.
fn trace_tx(
    setup: &[(Address, Vec<u8>, u64)],
    to: Address,
    input: Vec<u8>,
    gas: u64,
) -> (Block, ExecutionResult) {
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

    let result = {
        let mut evm = jenner_evm(FirehoseInspector::new(&mut tracer));
        {
            let _untraced = reth_firehose::suspend_tracing();
            for (to, input, gas) in setup {
                let result = evm.inspect_tx_commit(tx_env(*to, input, *gas)).expect("tx executes");
                assert!(result.is_success(), "setup transaction succeeds: {result:?}");
            }
        }
        let event = tx_event(setup.len() as u32, to, Bytes::from(input.clone()), gas);
        evm.inspector().tracer_mut().on_tx_start(event, None);
        evm.inspect_tx_commit(tx_env(to, &input, gas)).expect("tx executes")
    };

    let mut receipt = ReceiptData::new(
        setup.len() as u32,
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

fn tx_env(to: Address, input: &[u8], gas: u64) -> BscTxEnv {
    let tx = TxEnv::builder()
        .caller(ALICE)
        .chain_id(Some(CHAIN_ID))
        .gas_limit(gas)
        .gas_price(0)
        .kind(TxKind::Call(to))
        .data(Bytes::copy_from_slice(input))
        .build()
        .unwrap();
    BscTxEnv::new(tx)
}

/// State with the CAS20 registries planted and the token features opened.
fn jenner_db() -> InMemoryDB {
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
    db
}

/// A Jenner EVM over [`jenner_db`].
fn jenner_evm<I>(inspector: I) -> BscEvm<InMemoryDB, I> {
    let mut cfg = CfgEnv::new_with_spec(BscHardfork::Jenner).with_chain_id(CHAIN_ID);
    cfg.disable_nonce_check = true;
    cfg.disable_balance_check = true;
    let block = BlockEnv { timestamp: U256::from(1_800_000_000u64), ..Default::default() };
    BscEvm::new(EvmEnv::new(cfg, block.into()), jenner_db(), inspector, true, false)
}

fn tx_event(index: u32, to: Address, input: Bytes, gas: u64) -> TxEvent {
    TxEvent {
        tx_type: TxType::Legacy,
        hash: B256::repeat_byte(0x11 + index as u8),
        from: ALICE,
        to: Some(to),
        input,
        value: U256::ZERO,
        gas,
        gas_price: U256::ZERO,
        nonce: index as u64,
        index,
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
