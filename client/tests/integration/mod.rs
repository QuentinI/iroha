pub use iroha_config::{
    base::proxy::Builder,
    iroha::{Configuration, ConfigurationProxy},
};

mod add_account;
mod add_domain;
mod asset;
mod asset_propagation;
mod burn_public_keys;
mod config;
mod connected_peers;
mod events;
mod multiple_blocks_created;
mod multisignature_account;
mod multisignature_transaction;
mod non_mintable;
mod offline_peers;
mod pagination;
mod permissions;
mod queries;
mod query_errors;
mod restart_peer;
mod roles;
mod set_parameter;
mod sorting;
mod transfer_asset;
mod triggers;
mod tx_history;
mod tx_rollback;
mod unregister_peer;
mod unstable_network;
mod upgrade;
