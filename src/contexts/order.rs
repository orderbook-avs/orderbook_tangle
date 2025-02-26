use crate::contexts::client::AggregatorClient;
use blueprint_sdk::config::GadgetConfiguration;
use blueprint_sdk::macros::contexts::KeystoreContext;

#[derive(Clone, KeystoreContext)]
pub struct EigenOrderContext {
    pub client: AggregatorClient,
    #[config]
    pub std_config: GadgetConfiguration,
}