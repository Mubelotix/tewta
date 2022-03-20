pub use super::*;

#[derive(Debug, Clone, protocol_derive::Protocol)]
pub struct DhtValue {
    pub(super) data: String,
}

#[derive(Default)]
pub struct DhtStore {
    table: Mutex<BTreeMap<KeyID, Vec<DhtValue>>>,
}

impl DhtStore {
    pub(super) async fn get(&self, key: &KeyID) -> Option<Vec<DhtValue>> {
        let mut table = self.table.lock().await;
        let values = table.get(key);
        if let Some(values) = values {
            if values.is_empty() {
                table.remove(key);
                return None;
            }
        }
        values.cloned()
    }

    pub(super) async fn set(&self, key: KeyID, value: DhtValue) {
        let mut table = self.table.lock().await;
        table.entry(key).or_insert_with(Vec::new).push(value);
    }
}

#[derive(Debug)]
enum SingleProviderLookupError {
    FailedToConnect,
    IdentityMismatch,
    PacketTooLarge,
    UnexpectedPacket,
    RequestIdMismatch,
    IoError(std::io::Error),
    ProtocolError(protocol::Error),
    HandshakeError(HandshakeError),
}

impl From<std::io::Error> for SingleProviderLookupError {
    fn from(e: std::io::Error) -> Self {
        SingleProviderLookupError::IoError(e)
    }
}

impl From<protocol::Error> for SingleProviderLookupError {
    fn from(e: protocol::Error) -> Self {
        SingleProviderLookupError::ProtocolError(e)
    }
}

impl Node {
    async fn dht_lookup_on_already_connected_provider(&self, key: &KeyID, peer_id: &PeerID) -> Result<DhtLookupResult, SingleProviderLookupError> {
        // Send request
        let request_id = self.dht_req_counter.next();
        let p = Packet::FindDhtValue(FindDhtValuePacket {
            request_id,
            key: key.clone(),
            limit_peers: MAX_DHT_PEERS_RETURNED,
            limit_values: MAX_DHT_VALUES_RETURNED,
        });
        let resp_receiver = self.on_find_dht_value_resp_packet.listen().await;
        self.connections.send_packet(peer_id, p).await;

        // Wait for response
        let resp = loop {
            let (n, p) = resp_receiver.recv().await.unwrap();
            if p.request_id == request_id && &n == peer_id {
                break p;
            }
        };

        Ok(resp.result)
    }

    async fn dht_lookup_on_single_provider(&self, key: &KeyID, (peer_id, addr): (PeerID, String)) -> Result<DhtLookupResult, SingleProviderLookupError> {
        use SingleProviderLookupError::*;
        debug!(self.ll, "DHT lookup on single provider: {}", peer_id);

        if self.connections.contains(&peer_id).await {
            debug!(self.ll, "Already connected to peer: {}", peer_id);
            return self.dht_lookup_on_already_connected_provider(key, &peer_id).await;
        }

        // TODO [#39]: Handshake coherence
        // Here we are handshaking but we don't insert the node so it does not benefits from all features our node may provide.
        // It's ok but we have to tell the other node to not consider ourselves like a long-time node, but rather a short term connection that will only exchange one request and response.

        let s = connect(addr).await.ok_or(FailedToConnect)?;
        debug!(self.ll, "Connected to {}", peer_id);
        let mut r = self.handshake(s).await.map_err(HandshakeError)?;
        debug!(self.ll, "Handshake with {} completed", peer_id);

        if peer_id != r.their_peer_id {
            return Err(IdentityMismatch);
        }

        let (mut r, mut w) = r.stream.into_split();

        // TODO [#40]: AES encryption here

        // Send the lookup request
        debug!(self.ll, "Sending lookup request to {}", peer_id);
        let request_id = self.dht_req_counter.next();
        let p = Packet::FindDhtValue(FindDhtValuePacket {
            request_id,
            key: key.clone(),
            limit_peers: MAX_DHT_PEERS_RETURNED,
            limit_values: MAX_DHT_VALUES_RETURNED,
        });
        let p = p.raw_bytes(&PROTOCOL_SETTINGS)?;
        let plen = p.len() as u32;
        let mut plen_buf = [0u8; 4];
        plen_buf.copy_from_slice(&plen.to_be_bytes());
        w.write_all(&plen_buf).await?;
        w.write_all(&p).await?;

        // Receive the lookup response
        debug!(self.ll, "Waiting for response from {}", peer_id);
        let plen = r.read_u32().await?;
        if plen >= MAX_PACKET_SIZE {
            return Err(PacketTooLarge);
        }
        let mut p = Vec::with_capacity(plen as usize);
        unsafe {p.set_len(plen as usize)};
        r.read_exact(&mut p).await?;
        let p = Packet::from_raw_bytes(&p, &PROTOCOL_SETTINGS)?;
        let p = match p {
            Packet::FindDhtValueResp(p) => p,
            _ => return Err(UnexpectedPacket),
        };

        // TODO [#41]: Send Quit packet

        if p.request_id != request_id {
            return Err(RequestIdMismatch);
        }
        Ok(p.result)
    }

    pub(super) async fn dht_lookup(&self, key: KeyID) -> Option<Vec<DhtValue>> {
        debug!(self.ll, "DHT lookup: {}", key);

        let mut providers = self.connections.peers_with_addrs().await;
        providers.sort_by_key(|(peer_id, _)| peer_id.distance(&key));
        providers.reverse();

        let mut concurrent_lookups = Vec::new();

        loop {
            // Fill with new lookups
            debug!(self.ll, "refill");
            while concurrent_lookups.len() < KADEMLIA_ALPHA {
                if let Some(provider) = providers.pop() {
                    concurrent_lookups.push(Box::pin(self.dht_lookup_on_single_provider(&key, provider)));
                } else if concurrent_lookups.is_empty() {
                    warn!(self.ll, "Lookup failed, no providers");
                    return None;
                } else {
                    break;
                }
            }

            // Wait for any lookup to finish
            debug!(self.ll, "wait");
            let (first_result, _, other_lookups) = futures::future::select_all(concurrent_lookups).await;
            concurrent_lookups = other_lookups;
            match first_result {
                Ok(DhtLookupResult::Found(values)) => {
                    debug!(self.ll, "DHT lookup found {} values", values.len());
                    return Some(values);
                }
                Ok(DhtLookupResult::NotFound(peers)) => {
                    // TODO [#42]: Prevent DOS
                    // A peer could flood the lookup system with bad suggestions
                    
                    debug!(self.ll, "DHT lookup not found, but we have more {} peers", peers.len());
                    providers.extend(peers);
                    providers.sort_by_key(|(peer_id, _)| peer_id.distance(&key));
                    providers.dedup();
                    providers.reverse();
                }
                Err(e) => {
                    warn!(self.ll, "DHT lookup failed: {:?}", e);
                }
            }
        }
    }
}
