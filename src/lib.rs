use blueprint_sdk::alloy::primitives::{address, Address, U256};
use blueprint_sdk::alloy::rpc::types::Log;
use blueprint_sdk::alloy::sol;
use blueprint_sdk::config::GadgetConfiguration;
use blueprint_sdk::event_listeners::evm::EvmContractEventListener;
use blueprint_sdk::job;
use blueprint_sdk::logging::info;
use blueprint_sdk::macros::load_abi;
use blueprint_sdk::std::convert::Infallible;
use blueprint_sdk::std::sync::LazyLock;
use serde::{Deserialize, Serialize};

type ProcessorError =
    blueprint_sdk::event_listeners::core::Error<blueprint_sdk::event_listeners::evm::error::Error>;

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    #[derive(Debug, Serialize, Deserialize)]
    TangleTaskManager,
    "contracts/out/TangleTaskManager.sol/TangleTaskManager.json"
);

load_abi!(
    TANGLE_TASK_MANAGER_ABI_STRING,
    "contracts/out/TangleTaskManager.sol/TangleTaskManager.json"
);

pub static TASK_MANAGER_ADDRESS: LazyLock<Address> = LazyLock::new(|| {
    std::env::var("TASK_MANAGER_ADDRESS")
        .map(|addr| addr.parse().expect("Invalid TASK_MANAGER_ADDRESS"))
        .unwrap_or_else(|_| address!("0000000000000000000000000000000000000000"))
});

#[derive(Clone)]
pub struct ExampleContext {
    pub config: GadgetConfiguration,
}

/// Processes a limit order task
#[job(
    id = 0,
    params(price, amount, is_buy),
    event_listener(
        listener = EvmContractEventListener<ExampleContext, TangleTaskManager::NewTaskCreated>,
        instance = TangleTaskManager,
        abi = TANGLE_TASK_MANAGER_ABI_STRING,
        pre_processor = process_order_event,
    ),
)]
pub fn process_limit_order(
    context: ExampleContext,
    price: U256,
    amount: U256,
    is_buy: bool,
) -> Result<String, Infallible> {
    info!("Processing limit order: price={}, amount={}, is_buy={}", price, amount, is_buy);
    Ok(format!("Processed limit order: price={}, amount={}, is_buy={}", price, amount, is_buy))
}

/// Pre-processor for handling new order events
async fn process_order_event(
    (event, _log): (TangleTaskManager::NewTaskCreated, Log),
) -> Result<Option<(U256, U256, bool)>, ProcessorError> {
    let task = event.task;
    Ok(Some((
        U256::from(task.order.price),
        U256::from(task.order.amount),
        task.order.isBuy,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let config = GadgetConfiguration::default();
        let context = ExampleContext { config };
        let result = process_limit_order(context, U256::from(100), U256::from(10), true).unwrap();
        assert_eq!(result, "Processed limit order: price=100, amount=10, is_buy=true");
    }
}
