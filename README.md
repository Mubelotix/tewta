<h1 align="center">p2pnet</h1>

<p align="center">
    <a href="https://www.gnu.org/licenses/agpl-3.0"><img src="https://img.shields.io/badge/License-AGPL_v3-blue.svg" alt="License: GNU AGPL v3"></a>
    <img alt="Lines of code" src="https://img.shields.io/tokei/lines/github/Mubelotix/p2pnet">
    <img alt="GitHub last commit" src="https://img.shields.io/github/last-commit/Mubelotix/p2pnet?color=%23347d39" alt="last commit badge">
    <img src="https://wakatime.com/badge/user/6a4c28c6-c833-460a-815e-15ce48b15c25/project/cf07aa0b-1f3c-42ff-a3c1-67a97f3a9ffa.svg" alt="Wakatime badge">
    <img alt="GitHub closed issues" src="https://img.shields.io/github/issues-closed-raw/Mubelotix/p2pnet?color=%23347d39" alt="closed issues badge">
    <a href="https://codecov.io/gh/Mubelotix/p2pnet"><img src="https://codecov.io/gh/Mubelotix/p2pnet/branch/master/graph/badge.svg?token=4CF0P16V5S" alt="Code coverage badge"/></a>
</p>

<p align="center">Experimental peer-to-peer social network built with Rust 🦀</p>

An experimental peer-to-peer network using [Kademlia](https://en.wikipedia.org/wiki/Kademlia) as its [DHT](https://en.wikipedia.org/wiki/Distributed_hash_table).

The end goal is to achieve a fully functional distributed clone of Twitter.

## Design

P2pnet is like a traditional social network, except user profiles are not stored on a centralized server (nor on federated servers).
Instead, each user distributes his own profile and helps distribute the profile of his friends.
Thus, following someone does not only mean subscribing to their actions but also means that you publicly support their content and participate in its broadcasting.  
As on Twitter, users will see content from people they follow, content their friends like, reply and share.

_Note that all these behavior choices are specific to this particular implementation, and the network could very well have clients acting differently._

## Moderation

Content on p2pnet is censorship-resistant and cannot be moderated.
However, as said above, users **only** see content from people they trust, and from people trusted by the people they directly trust.
Thus thanks to the high control on content sources, malicious users and unwanted content will not reach users.

## Non-goals

- Care about NAT traversal. Users gotta have to fix their shitty network settings.*
- Implement username claiming. [ENS](https://ens.domains/) is fine.
- Service-wide moderation service. Users will moderate their own profile, and that's all.
- Decentralized search engine*

_* It's a non-goal for me, but *you* can PR._

## FAQ

### Why not use libp2p?

I could have used [libp2p](https://libp2p.io/), and that would have been amazing, but I decided not to for several reasons:
- Rusty libp2p has no testing framework
- The library lacks real-world experience and is still solely experimental
- I fear we could get blocked by missing features
- Building everything from scratch allows us to optimize as much as needed

### Why use Kademlia?

[Kademlia](https://en.wikipedia.org/wiki/Kademlia) is simple and works well.
An open protocol needs to be easy to understand.  
Note that this implementation is not compatible with other Kademlia nodes.
The concept remains, but the design has been adapted to this project.

## License

The following license applies to this software.  

_Note: As for the network protocol, it is not restricted in any way.
It can be implemented by anyone without restrictions, as long as if they copy code from this project they comply with the following license._

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
