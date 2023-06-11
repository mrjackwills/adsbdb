### 2023-06-11

### Chores
+ dependencies updated, [109c7a72ef21652b3269fd3a86c0d5842c3ebf70], [66eee54ede84af8cd0a5e18ae9c51186ebb3a724], [88fd8d7447d67c7eae879722c4669cf1032240f9], [c827db21e6375e964e9e39dfa48fced6964bdc27], [e94098e48cd5bafbf8b4fef93b06d52a65f4033e], [fcec134f6e05ddee4accfd7ead4e2c608e646c00]
+ Dockerfiles Alpine bump to 3.18, [4f55f7c63ce0fec02aacc6b18bcfc0a80bec2f2b]
+ sqlx prepare, [6057dce2c6a9fce7b5d3df72f907a1cd4d95f2fa], [e8139a9da2d344211a73e8cdf46703c453d940c4], [e355807c706cc807c61d82c8ddeb14039911d74f]

### Docs
+ CONTRIBUTING.md added, [676d06f6a7a1766b58d1b2bbd9f240afc0726956]
+ GitHub data issue add source, [4ae44c25716a29eb81e02c2142e4ac5fffa87f29]

### Features
+ `define_routes` macro, [8daf85854e49784294580f961ae0b9ae47444d44]
+ `sleep` macro, [21f1b81a2a02cbc8a170c52d4a7b3961ec067642]
+ `unit_struct` & `from_request_parts` macros, [61e2391c59ed36870b8aaa2640002db6b2024bc2], [6e52dc267bb06fc96341d73662ccff6dfb8a445e]
+ app_error internal! macro, [c3f497171b387c36a26429e100c0e2113124fd3a]
+ create_release prepare sqlx, [f05b6b15ee87c4e391bbf9758361d495e69415af]
+ devcontainer install sqlx-cli, [1403434d1f0e36452ba9f3f4d8b6923b06eb5225]
+ from_request_parts macro, [c5a5e2e6c49b4f40510099ae6a4815ce9e46f41b]
+ ModelAirport use macro, [c9d01503c5d18a20eb25cd7cf45b97cf3f128e68]
+ ModelAircraft use macros, [ab72f2eba49d768f9ef998cadd12cb1f91b895e6]
+ ModelAirline use macro, [6619fe9b4638d5d81e35a105092e1780b371164f]
+ ModelAirport query improved, [b9d603c85d856cb2b80cf02283e677872e59224a]
+ ModelFlightroute, use JOINS rather than subqueries, [3d31ec2a7bef3398d6b35352c3a43a09dc01bf84]

### Fixes
+ GitHub workflow use regex for semver tag, [de75904774749346af87b73e5eae0234c61b368a]
+ nursery redis drop lint, [535f1eee02bca5df9387803fc7bb64eede04e630]

### Refactors
+ dead code removed, [91a4a4043c0da7d0c68c132e6442e144aa30daf8]
+ methods renamed, [16337e76f140731c953f35fa71239fcb740803ff], [23686cecbe8eef8a6e702176098ee7faf187cf98]
+ ModelFlightRoute query refactors, and macros, [1c296a1b7fed5c9e83bf53501948c4a9ccd1f12f]
+ mods moved, [03606946cd0ad159a12dc40692df0d504c4ff4aa]
+ ModelFlightroute sql query into parts, [cb78968befa521b1589ba4635c89e1c2e9db84ad]
+ ModelFlightroute insert scraped flightroute use macros, [b64c34549e87f180f399f757a77edb05d4fe3659]

### Reverts
+ .devcontainer sparse protocol now default, [430ce92bebbc7e856612f80fcef754b507f2d426]


see <a href='https://github.com/mrjackwills/adsbdb/blob/main/CHANGELOG.md'>CHANGELOG.md</a> for more details
