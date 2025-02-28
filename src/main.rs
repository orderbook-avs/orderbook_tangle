use alloy_network::EthereumWallet;
use alloy_primitives::Address;
use alloy_signer_local::PrivateKeySigner;
use blueprint_sdk::logging::{info, warn};
use blueprint_sdk::runners::core::runner::BlueprintRunner;
use blueprint_sdk::runners::eigenlayer::bls::EigenlayerBLSConfig;
use blueprint_sdk::utils::evm::get_wallet_provider_http;
use ob_avs::constants::{
    AGGREGATOR_PRIVATE_KEY, TASK_MANAGER_ADDRESS,
};
use blueprint_sdk::alloy::primitives::{address, U256};

use ob_avs::contexts::aggregator::AggregatorContext;
use ob_avs::contexts::client::AggregatorClient;
use ob_avs::contexts::order::EigenOrderContext;
use ob_avs::jobs::create_order::OrderEigenEventHandler;
use ob_avs::jobs::initialize_task::InitializeBlsTaskEventHandler;
use ob_avs::OrderBookTaskManager;
use blueprint_sdk::utils::evm::get_provider_http;

#[blueprint_sdk::main(env)]
async fn main() {
    let signer: PrivateKeySigner = AGGREGATOR_PRIVATE_KEY
        .parse()
        .expect("failed to generate wallet ");
    let wallet = EthereumWallet::from(signer);
    let provider = get_wallet_provider_http(&env.http_rpc_endpoint, wallet.clone());

    let server_address = format!("{}:{}", "127.0.0.1", 8081);
    let eigen_order_context = EigenOrderContext {
        client: AggregatorClient::new(&server_address)?,
        std_config: env.clone(),
    };

    let aggregator_context =
        AggregatorContext::new(server_address, *TASK_MANAGER_ADDRESS, wallet, env.clone())
            .await
            .unwrap();

    // Printing out the task manager address
    info!("Task manager address: {}", *TASK_MANAGER_ADDRESS);

    let contract = OrderBookTaskManager::OrderBookTaskManagerInstance::new(
        *TASK_MANAGER_ADDRESS,
        provider,
    );

    let initialize_task =
        InitializeBlsTaskEventHandler::new(contract.clone(), aggregator_context.clone());

    let create_order = OrderEigenEventHandler::new(contract.clone(), eigen_order_context);
    let rpc_endpoint = env.http_rpc_endpoint.clone();    
    info!("Spawning a task to create a task on the contract...");
    blueprint_sdk::tokio::spawn(async move {                
        let provider = get_provider_http(&rpc_endpoint);
        let contract_task_generator = OrderBookTaskManager::new(*TASK_MANAGER_ADDRESS, provider);

        // We use the Anvil Account #4 as the Task generator address
        for _ in 1..3 {
            blueprint_sdk::tokio::time::sleep(std::time::Duration::from_secs(5)).await;

            let task = contract_task_generator
                .createNewTask(U256::from(5), U256::from(200), address!("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"), address!("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"), U256::from(5), 0, vec![].into())
                .from(address!("15d34AAf54267DB7D7c367839AAf71A00a2C6A65"));
            let receipt = task.send().await.unwrap().get_receipt().await.unwrap();
            if receipt.status() {
                info!("Task created successfully");
            } else {
                warn!("Task creation failed: {:?}", receipt);                            
            }
        }
    });

    info!("~~~ Executing the orderbook blueprint ~~~");
    let eigen_config = EigenlayerBLSConfig::new(Address::default(), Address::default());
    BlueprintRunner::new(eigen_config, env)
        .job(create_order)
        .job(initialize_task)
        .background_service(Box::new(aggregator_context))
        .run()
        .await?;

    info!("Exiting...");
    Ok(())
}