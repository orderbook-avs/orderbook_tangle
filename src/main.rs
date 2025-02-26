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

#[blueprint_sdk::main(env)]
async fn main() {
    let signer: PrivateKeySigner = AGGREGATOR_PRIVATE_KEY
        .parse()
        .expect("failed to generate wallet ");
    let wallet = EthereumWallet::from(signer);
    let provider = get_wallet_provider_http(&env.http_rpc_endpoint, wallet.clone());

    let signer_task_generator: PrivateKeySigner = "2a871d0798f97d79848a013d4936a73bf4cc922c825d33c1cf7073dff6d409c6"
        .parse()
        .expect("failed to generate wallet ");
    let wallet_task_generator = EthereumWallet::from(signer_task_generator);
    let provider_task_generator = get_wallet_provider_http(&env.http_rpc_endpoint, wallet_task_generator.clone());

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
    
    info!("Spawning a task to create a task on the contract...");
    blueprint_sdk::tokio::spawn(async move {                
        let contract_task = OrderBookTaskManager::new(*TASK_MANAGER_ADDRESS, provider_task_generator.clone());

        // We use the Anvil Account #4 as the Task generator address
        loop {
            blueprint_sdk::tokio::time::sleep(std::time::Duration::from_secs(5)).await;

            let task: alloy_contract::CallBuilder<alloy_transport::BoxTransport, &alloy_provider::RootProvider<alloy_transport::BoxTransport>, std::marker::PhantomData<OrderBookTaskManager::createNewTaskCall>> = contract_task
                .createNewTask(U256::from(5), U256::from(1), address!("0x0000000000000000000000000000000000000000"), U256::from(1), true, 100u32, vec![0].into())
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