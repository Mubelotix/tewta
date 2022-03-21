<h1 align="center">p2pnet</h1>

<p align="center">
    <img src="https://wakatime.com/badge/user/6a4c28c6-c833-460a-815e-15ce48b15c25/project/cf07aa0b-1f3c-42ff-a3c1-67a97f3a9ffa.svg" alt="Wakatime badge">
    <a href="https://www.gnu.org/licenses/agpl-3.0"><img src="https://img.shields.io/badge/License-AGPL_v3-blue.svg" alt="License: GNU AGPL v3"></a>
</p>

<p align="center">Experimental peer-to-peer social network built with Rust 🦀</p>

An experimental peer-to-peer network using [Kademlia](https://en.wikipedia.org/wiki/Kademlia) as its [DHT](https://en.wikipedia.org/wiki/Distributed_hash_table).

The end goal is to achieve a fully functional distributed clone of Twitter.

## Non-goals

- Care about NAT traversal. Users gotta have to fix their shitty network settings.*
- Implement username claiming. [ENS](https://ens.domains/) is fine.
- Service-wide moderation service. Users will moderate their own profile, and that's all.
- Decentralized search engine*

_* It's a non-goal for me, but *you* can PR._

## Why not use libp2p?
  
I could have used [libp2p](https://libp2p.io/) and that would have been amazing, but I decided not to for several reasons:
- Rusty libp2p has no testing framework
- The library lacks real-world experience and is fairly experimental
- It's quite a big library, so it's hard to fully understand how it works
- I want to be able to overoptimize things
- I fear I could get blocked by missing features
- It's a learning project after all, why would I want to use a library?

### 

    p2pnet; distributed social network
    Copyright (C) 2022  Mubelotix <mubelotix@gmail.com>

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU Affero General Public License as published
    by the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU Affero General Public License for more details.

    You should have received a copy of the GNU Affero General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.
