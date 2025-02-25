use ob_avs as blueprint;
use blueprint::{TangleTaskManager, TASK_MANAGER_ADDRESS};
use blueprint_sdk::alloy::primitives::{address, Address, U256};
use blueprint_sdk::logging::{info, warn};
use blueprint_sdk::macros::main;
use blueprint_sdk::runners::core::runner::BlueprintRunner;
use blueprint_sdk::runners::eigenlayer::bls::EigenlayerBLSConfig;
use blueprint_sdk::utils::evm::get_provider_http;

#[main(env)]
async fn main() {
    // Create your service context
    // Here you can pass any configuration or context that your service needs.
    let context = blueprint::ExampleContext {
        config: env.clone(),
    };

    // Get the provider
    let rpc_endpoint = env.http_rpc_endpoint.clone();
    let provider = get_provider_http(&rpc_endpoint);

    // Create an instance of the task manager
    let contract = TangleTaskManager::new(*TASK_MANAGER_ADDRESS, provider);

    // Create the event handler from the job
    let process_order_job = blueprint::ProcessLimitOrderEventHandler::new(contract, context.clone());

    // Spawn a task to create test orders
    info!("Spawning a task to create test orders...");
    blueprint_sdk::tokio::spawn(async move {
        let provider = get_provider_http(&rpc_endpoint);
        let contract = TangleTaskManager::new(*TASK_MANAGER_ADDRESS, provider);
        loop {
            blueprint_sdk::tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            
            // Create a test buy order
            let task = contract
                .createNewTask(
                    U256::from(100), // price
                    U256::from(10),  // amount
                    true,            // is_buy
                    100u32,          // quorum threshold
                    vec![0u8].into(), // quorum numbers
                )
                .from(address!("15d34AAf54267DB7D7c367839AAf71A00a2C6A65"));
            
            let receipt = task.send().await.unwrap().get_receipt().await.unwrap();
            if receipt.status() {
                info!("Buy order created successfully");
            } else {
                warn!("Buy order creation failed");
            }
        }
    });

    info!("Starting the event watcher ...");
    let eigen_config = EigenlayerBLSConfig::new(Address::default(), Address::default());
    BlueprintRunner::new(eigen_config, env)
        .job(process_order_job)
        .run()
        .await?;

    info!("Exiting...");
    Ok(())
}
