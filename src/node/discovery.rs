// Copyright (c) 2022  Mubelotix <mubelotix@gmail.com>
// Program licensed under GNU AGPL v3 or later. See the LICENSE file for details.

use crate::prelude::*;

impl Node {
    pub async fn discover_peers_in_bucket(&self, bucket_level: usize, bucket_id: usize) {
        assert!(bucket_level < 128 && bucket_id < 3);

        let target = self.peer_id.generate_in_bucket(bucket_level, bucket_id);
        let mut mask = vec![0xFFu8; bucket_level.div_euclid(4)];
        match bucket_level.rem_euclid(4) {
            0 => mask.push(0b11000000),
            1 => mask.push(0b11110000),
            2 => mask.push(0b11111100),
            3 => mask.push(0b11111111),
            _ => unsafe { unreachable_unchecked() },
        }

        let mut providers = self.connections.peers_on_bucket_and_under(bucket_level).await;
        let mut candidates: Vec<(PeerID, String)> = Vec::new();
        let mut old_candidates: BTreeSet<(PeerID, String)> = BTreeSet::new();
        let mut missing_peers = KADEMLIA_BUCKET_SIZE - self.connections.peers_on_bucket(bucket_level, bucket_id).await.len();

        while missing_peers > 0 {
            if let Some((peer_id, addr)) = candidates.pop() {
                // Assert we did not try that peer already
                if old_candidates.contains(&(peer_id.clone(), addr.clone())) {
                    continue;
                }
                old_candidates.insert((peer_id.clone(), addr.clone()));

                // Make sure this is a valid peer suggestion
                if self.connections.contains(&peer_id).await {
                    continue;
                }
                if !peer_id.matches(&target, &mask) {
                    warn!(self.ll, "Response contains peers that do not match request");
                }

                // TODO [#30]: close connection properly
                let (r, w) = match connect(addr).await {
                    Some(s) => s.into_split(),
                    None => continue,
                };
                let peer_id = match self.handshake(r, w, Some(peer_id)).await {
                    Ok(r) => r,
                    Err(e) => {
                        error!(self.ll, "Handshake failed: {:?}", e);
                        return;
                    }
                };
                trace!(self.ll, "Successfully discovered one peer ({})", peer_id);
                missing_peers -= 1;
            } else if let Some(provider) = providers.pop() {
                let request_id = self.discover_peer_req_counter.next();
                let p = Packet::DiscoverPeers(DiscoverPeersPacket {
                    request_id,
                    target: target.clone(),
                    mask: mask.clone(),
                    limit: MAX_DISCOVERY_PEERS_RETURNED,
                });
    
                // TODO [#31]: Add timeout
    
                let resp_receiver = self.on_discover_peers_resp_packet.listen().await;
                self.connections.send_packet(&provider, p).await;
    
                loop {
                    let (n, resp) = resp_receiver.recv().await.unwrap();
                    if resp.request_id == request_id && n == provider {
                        candidates = resp.peers;
                        break;
                    }
                }
            } else {
                trace!(self.ll, "No providers available");
                break;
            }
        }
    }
}
