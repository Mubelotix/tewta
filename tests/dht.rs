// Copyright (c) 2022  Mubelotix <mubelotix@gmail.com>
// Program licensed under GNU AGPL v3 or later. See the LICENSE file for details.

mod common;
use crate::common::*;
use tewta::prelude::*;

#[tokio::test]
async fn test_dht() {
    #[cfg(not(feature = "test"))]
    compile_error!("Test feature required");

    let (command_sender, nodes) = launch_network(500, false).await;

    // Wait for network to boot
    sleep(Duration::from_secs(10)).await;

    for _ in 0..2 {
        // Update buckets
        for node in &nodes {
            node.connections.refresh_buckets().await; // This will complete immediately so no need to spawn futures
        }

        // Wait for buckets to update
        sleep(Duration::from_secs(10)).await;
    }

    // One peer adds the entry to the DHT
    let key = nodes[0].peer_id.to_owned();
    nodes[0].dht.set(key.clone(), DhtValue {
        cached_addr: None,
        account_snapshot_desc: AccountSnapshotDescriptor {
            timestamp: 0,
            hash: Vec::new(),
        }.sign(&nodes[0].rsa_public_key, &nodes[0].rsa_private_key).unwrap(),
    }).await;
    sleep(Duration::from_secs(1)).await;

    // The other node fetches that entry
    nodes[454].dht_lookup(key).await.unwrap();
}
