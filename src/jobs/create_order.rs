#![allow(dead_code)]
use crate::contexts::client::SignedTaskResponse;
use crate::contexts::order::EigenOrderContext;
use crate::IOrderBookTaskManager::{TaskResponse, Order};
use crate::{
    OrderBookTaskManager, ProcessorError, ORDER_BOOK_TASK_MANAGER_ABI_STRING,
};
use alloy_primitives::{keccak256, Bytes, U256};
use alloy_sol_types::SolType;
use blueprint_sdk::contexts::keystore::KeystoreContext;
use blueprint_sdk::crypto::bn254::ArkBlsBn254;
use blueprint_sdk::event_listeners::evm::EvmContractEventListener;
use blueprint_sdk::keystore::backends::Backend;
use blueprint_sdk::logging::{error, info};
use blueprint_sdk::macros::ext::keystore::backends::bn254::Bn254Backend;
use blueprint_sdk::macros::job;
use color_eyre::Result;
use blueprint_sdk::eigensdk::crypto_bls::BlsKeyPair;
use blueprint_sdk::eigensdk::crypto_bls::OperatorId;
use std::convert::Infallible;

/// Sends a signed task response to the BLS Aggregator.
///
/// This job is triggered by the `NewTaskCreated` event emitted by the `OrderBookTaskManager`.
/// The job creates a limit order and sends the signed task response to the BLS Aggregator.
/// The job returns 1 if the task response was sent successfully.
/// The job returns 0 if the task response failed to send or failed to get the BLS key.
#[job(
    id = 0,
    params(order, orderbook, task_created_block, quorum_numbers, quorum_threshold_percentage, task_index),
    event_listener(
        listener = EvmContractEventListener<EigenOrderContext, OrderBookTaskManager::NewTaskCreated>,
        instance = OrderBookTaskManager,
        abi = ORDER_BOOK_TASK_MANAGER_ABI_STRING,
        pre_processor = convert_event_to_inputs,
    ),
)]
pub async fn order_eigen(
    ctx: EigenOrderContext,
    order: Order,
    orderbook: Vec<Order>,
    task_created_block: u32,
    quorum_numbers: Bytes,
    quorum_threshold_percentage: u8,
    task_index: u32,
) -> std::result::Result<u32, Infallible> {
    let client = ctx.client.clone();

    info!("Finding matches for task index: {}", task_index);

    let mut new_order = order.clone();
    let mut new_other_order = order.clone();
    let mut matched_order_index = U256::from(0);

    for (index, other_order) in orderbook.iter().enumerate() {
        if order.user == other_order.user {
            continue;
        }

        if order.isFilled || other_order.isFilled {
            continue;
        }

        if other_order.token_owned == order.token_owned {
            continue;
        } 
        
        let price_for_user = order.amount_owned / order.amount_not_owned;
        let price_for_other_user = other_order.amount_owned / other_order.amount_not_owned;   

        if (price_for_other_user < price_for_user) {
            continue;
        }
        
        let diff = if price_for_user >= price_for_other_user {
            price_for_user - price_for_other_user
        } else {
            price_for_other_user - price_for_user
        };

        let percentage_difference = (diff / ((price_for_user + price_for_other_user) / alloy_primitives::Uint::from(2))) * alloy_primitives::Uint::from(100);
        if percentage_difference > order.slippage {
            continue;
        }

        matched_order_index = U256::from(index);

        if other_order.amount_not_owned == order.amount_not_owned {            
            new_order.isFilled = true;       
            new_order.amount_not_owned = U256::from(0);                 
            new_other_order.isFilled = true;
            new_other_order.amount_not_owned = U256::from(0);
        }

        if other_order.amount_not_owned > order.amount_not_owned {
            new_order.isFilled = true;
            new_other_order.isPartiallyFilled = true;
            new_order.amount_not_owned = U256::from(0);
            new_other_order.amount_not_owned = other_order.amount_not_owned - order.amount_not_owned;
        }

        if other_order.amount_not_owned < order.amount_not_owned {
            new_other_order.isFilled = true;
            new_other_order.amount_not_owned = U256::from(0);
            new_order.isPartiallyFilled = true;
            new_order.amount_not_owned = order.amount_not_owned - other_order.amount_not_owned;
        }

        break;
    }

    // Create a TaskResponse object
    let task_response = TaskResponse {
        referenceTaskIndex: task_index,        
        newOrder: new_order,
        newOtherOrder: new_other_order,      
        matchedOrderIndex: matched_order_index,  
    };

    // info!("The task response is {:#?}", task_response);

    let bn254_public = ctx.keystore().first_local::<ArkBlsBn254>().unwrap();
    let bn254_secret = match ctx.keystore().expose_bls_bn254_secret(&bn254_public) {
        Ok(s) => match s {
            Some(s) => s,
            None => return Ok(0),
        },
        Err(_) => return Ok(0),
    };
    let bls_key_pair = match BlsKeyPair::new(bn254_secret.0.to_string()) {
        Ok(pair) => pair,
        Err(e) => return Ok(0),
    };
    let operator_id = operator_id_from_key(bls_key_pair.clone());

    // info!("The operator ID is {}", operator_id);

    // Sign the Hashed Message and send it to the BLS Aggregator
    let msg_hash = keccak256(<TaskResponse as SolType>::abi_encode(&task_response));

    // info!("The message hash is {:#?}", msg_hash);

    let signed_response = SignedTaskResponse {
        task_response,
        signature: bls_key_pair.sign_message(msg_hash.as_ref()),
        operator_id,
    };

    info!(
        "Sending signed task response to BLS Aggregator: {:#?}",
        signed_response
    );
    if let Err(e) = client.send_signed_task_response(signed_response).await {
        error!("Failed to send signed task response: {:?}", e);
        return Ok(0);
    }

    Ok(1)
}

/// Generate the Operator ID from the BLS Keypair
pub fn operator_id_from_key(key: BlsKeyPair) -> OperatorId {
    let pub_key = key.public_key();
    let pub_key_affine = pub_key.g1();

    let x_int: num_bigint::BigUint = pub_key_affine.x.into();
    let y_int: num_bigint::BigUint = pub_key_affine.y.into();

    let x_bytes = x_int.to_bytes_be();
    let y_bytes = y_int.to_bytes_be();

    keccak256([x_bytes, y_bytes].concat())
}

/// Converts the event to inputs.
///
/// Uses a tuple to represent the return type because
/// the macro will index all values in the #[job] function
/// and parse the return type by the index.
pub async fn convert_event_to_inputs(
    (event, _log): (
        OrderBookTaskManager::NewTaskCreated,
        alloy_rpc_types::Log,
    ),
) -> Result<Option<(Order, Vec<Order>, u32, Bytes, u8, u32)>, ProcessorError> {
    let task_index = event.taskIndex;
    let order = event.task.order;
    let orderbook = event.task.orderbook;
    let task_created_block = event.task.taskCreatedBlock;
    let quorum_numbers = event.task.quorumNumbers;
    let quorum_threshold_percentage = event.task.quorumThresholdPercentage.try_into().unwrap();
    Ok(Some((
        order,
        orderbook,
        task_created_block,
        quorum_numbers,
        quorum_threshold_percentage,
        task_index,
    )))
}