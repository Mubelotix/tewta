// Copyright (c) 2022  Mubelotix <mubelotix@gmail.com>
// Program licensed under GNU AGPL v3 or later. See the LICENSE file for details.

mod common;
use crate::common::*;
use p2pnet::{commands::*, RUNNING_COMMAND_COUNTER};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_discovery() {
    #[cfg(not(feature = "test"))]
    compile_error!("Test feature required");

    let nodes = launch_network(50, false).await.1;

    // Wait for network to boot
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    // Update buckets
    for node in &nodes {
        node.connections.refresh_buckets().await; // This will complete immediately so no need to spawn futures
    }

    // Wait for buckets to update
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    // TODO [#50]: Add a few assertions in the discovery test
}
