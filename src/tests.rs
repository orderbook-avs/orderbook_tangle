use crate::constants::{AGGREGATOR_PRIVATE_KEY, TASK_MANAGER_ADDRESS};
use crate::contexts::aggregator::AggregatorContext;
use crate::contexts::client::AggregatorClient;
use crate::contexts::order::EigenOrderContext;
use crate::jobs::create_order::OrderEigenEventHandler;
use crate::jobs::initialize_task::InitializeBlsTaskEventHandler;
use crate::OrderBookTaskManager;
use alloy_contract::{CallBuilder, CallDecoder};
use alloy_network::{EthereumWallet, Ethereum};
use alloy_primitives::{address, Address, Bytes, U256};
use alloy_provider::Provider;
use alloy_rpc_types::TransactionReceipt;
use alloy_signer_local::PrivateKeySigner;
use alloy_sol_types::sol;
use alloy_transport::Transport;
use blueprint_sdk::logging::{error, info, setup_log};
use blueprint_sdk::runners::eigenlayer::bls::EigenlayerBLSConfig;
use blueprint_sdk::testing::utils::eigenlayer::runner::EigenlayerBLSTestEnv;
use blueprint_sdk::testing::utils::eigenlayer::EigenlayerTestHarness;
use blueprint_sdk::testing::utils::harness::TestHarness;
use blueprint_sdk::testing::utils::runner::TestEnv;
use blueprint_sdk::utils::evm::{get_provider_http, get_provider_ws, get_wallet_provider_http};
use futures::StreamExt;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

sol!(
    #[allow(missing_docs, clippy::too_many_arguments)]
    #[sol(rpc)]
    #[derive(Debug)]
    RegistryCoordinator,
    "./contracts/out/RegistryCoordinator.sol/RegistryCoordinator.json"
);

#[tokio::test(flavor = "multi_thread")]
async fn test_eigenlayer_orderbook_blueprint() {
    run_eigenlayer_orderbook_test(true, 1).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn test_eigenlayer_pre_register_orderbook_blueprint() {
    run_eigenlayer_orderbook_test(false, 1).await;
}

async fn run_eigenlayer_orderbook_test(
    exit_after_registration: bool,
    expected_responses: usize,
) {
    setup_log();

    // Initialize test harness
    let temp_dir = tempfile::TempDir::new().unwrap();
    let harness = EigenlayerTestHarness::setup(temp_dir).await.unwrap();

    let env = harness.env().clone();
    let http_endpoint = harness.http_endpoint.to_string();

    // Deploy Task Manager
    let task_manager_address = deploy_task_manager(&harness).await;

    // Spawn Task Spawner and Task Response Listener
    let successful_responses = Arc::new(Mutex::new(0));
    let successful_responses_clone = successful_responses.clone();
    let response_listener_address =
        setup_task_response_listener(&harness, task_manager_address, successful_responses.clone())
            .await;
    let task_spawner = setup_task_spawner(&harness, task_manager_address).await;
    tokio::spawn(async move {
        task_spawner.await;
    });
    tokio::spawn(async move {
        response_listener_address.await;
    });

    info!("Starting Blueprint Execution...");
    let signer: PrivateKeySigner = AGGREGATOR_PRIVATE_KEY
        .parse()
        .expect("failed to generate wallet ");
    let wallet = EthereumWallet::from(signer);
    let provider = get_wallet_provider_http(&http_endpoint, wallet.clone());

    // Create aggregator
    let server_address = format!("{}:{}", "127.0.0.1", 8081);
    let eigen_client_context = EigenOrderContext {
        client: AggregatorClient::new(&server_address).unwrap(),
        std_config: env.clone(),
    };
    let aggregator_context =
        AggregatorContext::new(server_address, *TASK_MANAGER_ADDRESS, wallet, env.clone())
            .await
            .unwrap();
    let aggregator_context_clone = aggregator_context.clone();

    // Create jobs
    let contract = OrderBookTaskManager::OrderBookTaskManagerInstance::new(
        task_manager_address,
        provider,
    );
    let initialize_task =
        InitializeBlsTaskEventHandler::new(contract.clone(), aggregator_context.clone());
    let order_eigen = OrderEigenEventHandler::new(contract.clone(), eigen_client_context);

    let mut test_env = EigenlayerBLSTestEnv::new(
        EigenlayerBLSConfig::new(Default::default(), Default::default())
            .with_exit_after_register(exit_after_registration),
        env.clone(),
    )
    .unwrap();
    test_env.add_job(initialize_task);
    test_env.add_job(order_eigen);
    test_env.add_background_service(aggregator_context);

    if exit_after_registration {
        // Run the runner once to register, since pre-registration is enabled
        test_env.run_runner().await.unwrap();
    }

    tokio::time::sleep(Duration::from_secs(2)).await;

    test_env.run_runner().await.unwrap();

    // Wait for the process to complete or timeout
    let timeout_duration = Duration::from_secs(300);
    let result = wait_for_responses(
        successful_responses.clone(),
        expected_responses,
        timeout_duration,
    )
    .await;

    // // Start the shutdown/cleanup process
    aggregator_context_clone.shutdown().await;

    // Clean up the ./db directory
    let _ = std::fs::remove_dir_all("./db");

    match result {
        Ok(Ok(())) => {
            info!("Test completed successfully with {expected_responses} tasks responded to.");
        }
        _ => {
            panic!(
                "Test failed with {} successful responses out of {} required",
                successful_responses_clone.lock().unwrap(),
                expected_responses
            );
        }
    }
}

pub async fn deploy_task_manager(harness: &EigenlayerTestHarness) -> Address {
    let env = harness.env().clone();
    let http_endpoint = &env.http_rpc_endpoint;
    let registry_coordinator_address = harness
        .eigenlayer_contract_addresses
        .registry_coordinator_address;
    let pauser_registry_address = harness.pauser_registry_address;
    let owner_address = harness.owner_account();
    let aggregator_address = harness.aggregator_account();
    let task_generator_address = harness.task_generator_account();

    let provider = get_provider_http(http_endpoint);
    let deploy_call = OrderBookTaskManager::deploy_builder(
        provider.clone(),
        registry_coordinator_address,
        10u32,
    );
    info!("Deploying Incredible Squaring Task Manager");
    let task_manager_address = match get_receipt(deploy_call).await {
        Ok(receipt) => match receipt.contract_address {
            Some(address) => address,
            None => {
                error!("Failed to get contract address from receipt");
                panic!("Failed to get contract address from receipt");
            }
        },
        Err(e) => {
            error!("Failed to get receipt: {:?}", e);
            panic!("Failed to get contract address from receipt");
        }
    };
    info!(
        "Deployed Incredible Squaring Task Manager at {}",
        task_manager_address
    );
    std::env::set_var("TASK_MANAGER_ADDRESS", task_manager_address.to_string());

    let task_manager = OrderBookTaskManager::new(task_manager_address, provider.clone());
    // Initialize the Order Book Task Manager
    info!("Initializing Order Book Task Manager");
    let init_call = task_manager.initialize(
        pauser_registry_address,
        owner_address,
        aggregator_address,
        task_generator_address,
    );
    let init_receipt = get_receipt(init_call).await.unwrap();
    assert!(init_receipt.status());
    info!("Initialized Order Book Task Manager");

    task_manager_address
}

pub async fn setup_task_spawner(
    harness: &EigenlayerTestHarness,
    task_manager_address: Address,
) -> impl std::future::Future<Output = ()> {
    let registry_coordinator_address = harness
        .eigenlayer_contract_addresses
        .registry_coordinator_address;
    let _task_generator_address = harness.task_generator_account();
    let accounts = harness.accounts().to_vec();
    let http_endpoint = harness.http_endpoint.to_string();

    let provider = get_provider_http(http_endpoint.as_str());
    let task_manager = OrderBookTaskManager::new(task_manager_address, provider.clone());
    let registry_coordinator =
        RegistryCoordinator::new(registry_coordinator_address, provider.clone());

    let operators = vec![vec![accounts[0]]];
    let quorums = Bytes::from(vec![0, 1, 2]); // Using multiple quorum numbers to ensure non-empty array
    async move {
        loop {
            // Increased delay to allow for proper task initialization
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;

            info!("Creating a new task...");
            let create_task_receipt = get_receipt(
                task_manager
                    .createNewTask(U256::from(5), U256::from(1), address!("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"), U256::from(1), true, 100u32, quorums.clone())
                    .from(address!("15d34AAf54267DB7D7c367839AAf71A00a2C6A65"))
            )
            .await;

            match create_task_receipt {
                Ok(receipt) => {
                    if receipt.status() {
                        info!("Created a new task successfully");
                    } else {
                        error!("Failed to create task - transaction reverted: {:?}", receipt);
                    }
                },
                Err(e) => {
                    error!("Failed to get receipt for create task: {:?}", e);
                }
            }

            if get_receipt(
                registry_coordinator.updateOperatorsForQuorum(operators.clone(), quorums.clone()),
            )
            .await
            .unwrap()
            .status()
            {
                info!("Updated operators for quorum...");
            }

            // Wait for task initialization to complete
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;

            tokio::process::Command::new("sh")
                .arg("-c")
                .arg(format!(
                    "cast rpc anvil_mine 1 --rpc-url {} > /dev/null",
                    http_endpoint
                ))
                .output()
                .await
                .unwrap();
            info!("Mined a block...");
        }
    }
}

pub async fn setup_task_response_listener(
    harness: &EigenlayerTestHarness,
    task_manager_address: Address,
    successful_responses: Arc<Mutex<usize>>,
) -> impl std::future::Future<Output = ()> {
    let ws_endpoint = harness.ws_endpoint.to_string();

    let task_manager = OrderBookTaskManager::new(
        task_manager_address,
        get_provider_ws(ws_endpoint.as_str()).await,
    );

    async move {
        let filter = task_manager.TaskResponded_filter().filter;
        let mut event_stream = match task_manager.provider().subscribe_logs(&filter).await {
            Ok(stream) => stream.into_stream(),
            Err(e) => {
                error!("Failed to subscribe to logs: {:?}", e);
                return;
            }
        };
        while let Some(event) = event_stream.next().await {
            let OrderBookTaskManager::TaskResponded {
                taskResponse: _, ..
            } = event
                .log_decode::<OrderBookTaskManager::TaskResponded>()
                .unwrap()
                .inner
                .data;
            let mut counter = match successful_responses.lock() {
                Ok(guard) => guard,
                Err(e) => {
                    error!("Failed to lock successful_responses: {}", e);
                    return;
                }
            };
            *counter += 1;
        }
    }
}

pub async fn get_receipt<T, P, D>(
    call: CallBuilder<T, P, D, Ethereum>,
) -> Result<TransactionReceipt, String>
where
    T: Transport + Clone,
    P: Provider<T, Ethereum>,
    D: CallDecoder,
{
    let pending_tx = match call.send().await {
        Ok(tx) => tx,
        Err(e) => {
            error!("Failed to send transaction: {:?}", e);
            return Err(e.to_string());
        }
    };

    let receipt = match pending_tx.get_receipt().await {
        Ok(receipt) => receipt,
        Err(e) => {
            error!("Failed to get transaction receipt: {:?}", e);
            return Err(e.to_string());
        }
    };

    Ok(receipt)
}

pub async fn wait_for_responses(
    successful_responses: Arc<Mutex<usize>>,
    task_response_count: usize,
    timeout_duration: Duration,
) -> Result<Result<(), String>, tokio::time::error::Elapsed> {
    tokio::time::timeout(timeout_duration, async move {
        loop {
            let count = match successful_responses.lock() {
                Ok(guard) => *guard,
                Err(_e) => {
                    return Err("Error while waiting for responses".to_string());
                }
            };
            if count >= task_response_count {
                info!("Successfully received {} task responses", count);
                return Ok(());
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    })
    .await
}