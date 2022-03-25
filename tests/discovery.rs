// Copyright (c) 2022  Mubelotix <mubelotix@gmail.com>
// Program licensed under GNU AGPL v3 or later. See the LICENSE file for details.

mod common;
use crate::common::*;

#[tokio::test]
async fn test_discovery() {
    #[cfg(not(feature = "test"))]
    compile_error!("Test feature required");

    let nodes = launch_network(50, false).await.1;

    // Wait for network to boot
    sleep(Duration::from_secs(5)).await;

    // Update buckets
    for node in &nodes {
        node.connections.refresh_buckets().await; // This will complete immediately so no need to spawn futures
    }

    // Wait for buckets to update
    sleep(Duration::from_secs(5)).await;

    // TODO [#50]: Add a few assertions in the discovery test
}
