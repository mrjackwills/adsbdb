# <a href='https://github.com/mrjackwills/adsbdb/releases/tag/v0.1.0'>v0.1.0</a>
### 2023-01-14

**This release has breaking changes in the response Aircraft JSON object**

### Chores
+ dependencies updated, [680af9c7](https://github.com/mrjackwills/adsbdb/commit/680af9c7d94e2bb00b79a3e3e77f4058eeea4977), [227cb14a](https://github.com/mrjackwills/adsbdb/commit/227cb14a1aef740d818654a2dc20a85877e0cf1c)

### Debug
+ ratelimit tracing, [f68df99c](https://github.com/mrjackwills/adsbdb/commit/f68df99caf4bb533afa1daf9439e593de25a8f92)

### Features
**Breaking Change**
+ `n_number` is now `registration`, the api now returns, or attempts to return, a registration for every aircraft, closes [#13](https://github.com/mrjackwills/adsbdb/issues/13). The `/aircraft/x` route now accepts either mode_s hex code or aircraft registration for `x`, [b468fa82](https://github.com/mrjackwills/adsbdb/commit/b468fa824575322e64142ed031b9de158c46fb52)

### Fixes
+ Use a reqwest::Client builder, to enable request timeout, as well as gzip & brotli, [57bd31d9](https://github.com/mrjackwills/adsbdb/commit/57bd31d95501c8ae6b1bc4ca88f92035ce137450)

# <a href='https://github.com/mrjackwills/adsbdb/releases/tag/v0.0.19'>v0.0.19</a>
### 2023-01-04

### Chores
+ dependencies updated, [f3c435d0](https://github.com/mrjackwills/adsbdb/commit/f3c435d0c30d6e88dab8ab527f441a734a60d05e), [356650c9](https://github.com/mrjackwills/adsbdb/commit/356650c90a2fe5159a26a90302b9345fc52a6a3e), [97242ae6](https://github.com/mrjackwills/adsbdb/commit/97242ae68b8c6dbe53639485a8c1134ff455e613)

### Refactors
+ put tracing_level into app_env, [4174a24f](https://github.com/mrjackwills/adsbdb/commit/4174a24fbbbd066d8439c06ed01ca65bfde84d0e)
+ ratelimit into separate file, [da31646c](https://github.com/mrjackwills/adsbdb/commit/da31646c204054b0a399dcc7d925184aa8c60f93), [c33449b6](https://github.com/mrjackwills/adsbdb/commit/c33449b69800e29dcfe916cea6d35ea0293df7f6)
+ get_cache simplifying, [b360492f](https://github.com/mrjackwills/adsbdb/commit/b360492ff53b330ddd45ed94a7e36772cbe906d0)

# <a href='https://github.com/mrjackwills/adsbdb/releases/tag/v0.0.18'>v0.0.18</a>
### 2022-12-16

### Chores
+ dependencies updated, [b8355f7f](https://github.com/mrjackwills/adsbdb/commit/b8355f7f5b9b362c9a1ace242a8b15a1eebc8121), [16ba8d13](https://github.com/mrjackwills/adsbdb/commit/16ba8d135740c29bcc2c4208fbab6a522fce3bbb), [343f13ec](https://github.com/mrjackwills/adsbdb/commit/343f13ec80b9b9edbb3514b7c8bb0b86f92c3cd4)
+ container alpine version bump, [5652a1b0](https://github.com/mrjackwills/adsbdb/commit/5652a1b0424071885b1ccf4b09db15ceb982a404)

### Features
+ api Dockerfile(s) use ubuntu container, [50d7b760](https://github.com/mrjackwills/adsbdb/commit/50d7b760cf67c6d0c28beee29b78dc9a947dc2ba)
+ rust caching in github action, [30cce60a](https://github.com/mrjackwills/adsbdb/commit/30cce60a2e5cbf6616bf9e649a4cfb3bcfa46e5a)

### Fixes
* lock redis in ratelimit methods, [e8304d30](https://github.com/mrjackwills/adsbdb/commit/e8304d308270bb2c178ffa5315771519f82552bf)

### Refactors
+ ttl turbofish into usize, [432892a9](https://github.com/mrjackwills/adsbdb/commit/432892a90faf15b106ebeae1a4ffd23edbdf8314)
+ Rust 1.66 linting, [873bbb29](https://github.com/mrjackwills/adsbdb/commit/873bbb29118d15a43352606750799422668a0918)

# <a href='https://github.com/mrjackwills/adsbdb/releases/tag/v0.0.17'>v0.0.17</a>
### 2022-11-25

### Chores
+ aggressive linting with rust 1.65.0, [755644bb](https://github.com/mrjackwills/adsbdb/commit/755644bb5fc2f984b87779c1d140117ed77c03b9)
+ dependencies updated, implement axum v0.6 changes, [c7812986](https://github.com/mrjackwills/adsbdb/commit/c781298636cd967df7a21fe302a07a6bf6811cee)
+ postgres upgraded to v15, closes [#7](https://github.com/mrjackwills/adsbdb/issues/7), [a59dfb85](https://github.com/mrjackwills/adsbdb/commit/a59dfb850fe2f01c0deeac27070c08ee2a0e388c)

### Docs
+ readme updated, [43f8f003](https://github.com/mrjackwills/adsbdb/commit/43f8f003114f86a08210cbe6bc9f406ef4f0f692)

### Features
+ use dtolnay/rust-toolchain in github workflow, [10e872b1](https://github.com/mrjackwills/adsbdb/commit/10e872b190c12658e7a9df02832e62445f5cad8e)
+ replace dotenv with dotenvy, [2234f3f8](https://github.com/mrjackwills/adsbdb/commit/2234f3f85c884ef98a4ae29a41e97fd4da42eee8)
+ replace lazy_static with once_cell, [524b3ef0](https://github.com/mrjackwills/adsbdb/commit/524b3ef06184fca94b1ce5d4569d1280f5f21b36)

### Fixes
+ typo Scrapper > Scraper, [d9ff9e3d](https://github.com/mrjackwills/adsbdb/commit/d9ff9e3de546fae58b37e5a546d630930bb013b1)

### Reverts
+ remove Cache<T>, just use Option<T>, [cce579cb](https://github.com/mrjackwills/adsbdb/commit/cce579cb41c4619a6fa109d9d6a40b3ebc9544de)


# <a href='https://github.com/mrjackwills/adsbdb/releases/tag/v0.0.16'>v0.0.16</a>
### 2022-10-15

### Chores
+ create_release.sh v0.1.1, [065daa3d](https://github.com/mrjackwills/adsbdb/commit/065daa3d6a4efa28a75bb7fe97ed2c94c426966d),

### Docs
+ readme updated, [94e1ea83](https://github.com/mrjackwills/adsbdb/commit/94e1ea83d36802876f072695065aac5df02f2c38),
+ comment typos fixed, [9d799ca3](https://github.com/mrjackwills/adsbdb/commit/9d799ca37aa968a228efe77667c6e0084d1305f0),
+ comments updated, [b1ccd36e](https://github.com/mrjackwills/adsbdb/commit/b1ccd36e6615922a8f6bf543a882fbf21f510195),

### Features
+ NNumber, ModeS & Callsign new_types, [6a42752e](https://github.com/mrjackwills/adsbdb/commit/6a42752e3395a837fb1abc42e73768d5ec2b583c),
+ Exit with code 1 if no .env file found, [0939a2d3](https://github.com/mrjackwills/adsbdb/commit/0939a2d38ffd633a029eee6e95c21062249e3e45),
+ UnknownAC (Aircraft/Callsign) enum, [596cfa07](https://github.com/mrjackwills/adsbdb/commit/596cfa07ac0704aaa3f8efd7d07d029e2a732c0c),

### Fixes
+ ttl as isize instead of usize, [84dbbf02](https://github.com/mrjackwills/adsbdb/commit/84dbbf02d66bfbf6b529f82a694ea4bb3946d419),
+ Cargo.lock tracked, [366a90ee](https://github.com/mrjackwills/adsbdb/commit/366a90eeb52122ec46b18395d2aac83363178be7),
+ cache aircraft/flightroute with Cache enum, [5118d3b4](https://github.com/mrjackwills/adsbdb/commit/5118d3b42a043e72a02a9f31daaf7ca7608f4b5c),
+ SQL query uppercase SQL reserved words, [e918e88c](https://github.com/mrjackwills/adsbdb/commit/e918e88c022d3d8b903156ccc82a03387edc07d5),
+ website added Aircraft Value table, [5ef5e1c7](https://github.com/mrjackwills/adsbdb/commit/5ef5e1c7063eba42a3728ef2fcf94de42cd93258),
+ try/catch on website script, [94be3918](https://github.com/mrjackwills/adsbdb/commit/94be3918d43719431960c2479fa0df57adec2743),
+ print version number when starting, [19de8a9d](https://github.com/mrjackwills/adsbdb/commit/19de8a9d2c8a4c3ae2dd0b8cb2e01722130e5f00),

### Refactors
+ get_addr() for creating axum usable address from &env fix: get_api_version use spit instead of chars, [e6657a0a](https://github.com/mrjackwills/adsbdb/commit/e6657a0a2225c0a1eb4ce4bacaa65b6e7d96eac0),
+ photo_scraper return Option<T> instead of Result<Option<T>, ApiError>, [842adfc9](https://github.com/mrjackwills/adsbdb/commit/842adfc90212431a0aedf7368520909264d21a65),
+ dead code removed, [97e57f11](https://github.com/mrjackwills/adsbdb/commit/97e57f116349beef4cd62a5daf67f6bcb6cd4753),, [321db5d9](https://github.com/mrjackwills/adsbdb/commit/321db5d9ac5cc2b4ffde04a69bd9d5ea3ce3cbc2),
+ sql query syntax tweaked, [40e5b5d0](https://github.com/mrjackwills/adsbdb/commit/40e5b5d0fd403a4d60edaff4ef1648cb4878ad10),
+ AppError into own module, [fbc6be14](https://github.com/mrjackwills/adsbdb/commit/fbc6be1428544c4b7e0425273a35640c205ab32d),

# <a href='https://github.com/mrjackwills/adsbdb/releases/tag/v0.0.15'>v0.0.15</a>
### 2022-09-07

### Chores
+ Dependencies - tokio updated, anyhow removed, [83c361d0](https://github.com/mrjackwills/adsbdb/commit/83c361d0e86a94108480b7b22b940d9f631d69f8),
+ dev container updated, from buster to bullseye, [8ec1b8e9](https://github.com/mrjackwills/adsbdb/commit/8ec1b8e9d47de8a501905a17717d97ffef26859a),

### Features
+ store cache values in using Redis Hashes, [42871bec](https://github.com/mrjackwills/adsbdb/commit/42871bec7941178467cedc44923dc79ba783a391),
+ website response explanation added Midpoint, [7639eec2](https://github.com/mrjackwills/adsbdb/commit/7639eec2dabde108160b19649f4f3ae040171c25),

### Fixes
+ postgres queries use uppercase text for SQL lang, [8ad0d341](https://github.com/mrjackwills/adsbdb/commit/8ad0d341e7e26d7b0e6ad0ad9205146ba9fc7c1e),
+ N_Number functions replace unwrwaps() with map_or_else, [aabcdda0](https://github.com/mrjackwills/adsbdb/commit/aabcdda0f5eeac2478c0b2bc372d8bd8e5143860),

### Refactors
+ ModelAircraft use &self, instead of &Self, as param, [4c391778](https://github.com/mrjackwills/adsbdb/commit/4c39177852ca9d82e492cbd26bf7c6ce4a4c1669),
+ Redis, key.to_string() once, and optional_null turbofish matching [85bbb6bf](https://github.com/mrjackwills/adsbdb/commit/85bbb6bfa423bde8ec865d8c745e1a1a014f4b1d),

# <a href='https://github.com/mrjackwills/adsbdb/releases/tag/v0.0.14'>v0.0.14</a>
### 2022-08-12

### Chores
+ aggressive linting, [0fa03c92](https://github.com/mrjackwills/adsbdb/commit/0fa03c922b0dedeb1c2a35ea8d49f7e456d06dc7),
+ dev docker container updated, [95faeef0](https://github.com/mrjackwills/adsbdb/commit/95faeef0ab6888b423cb2fba33126f4367149f9b), [ae77cb62](https://github.com/mrjackwills/adsbdb/commit/ae77cb62039cdbd1321604df5b60530c7378f7fa),
+ dependencies updated, [7b82d9d7](https://github.com/mrjackwills/adsbdb/commit/7b82d9d7a609e00674d59f158da497fdd74b3422),

### Docs
+ readme updated, [24037f9d](https://github.com/mrjackwills/adsbdb/commit/24037f9d36be22d3907361f62a5c5c61461af537),

### Features
+ api.Dockerfile switch from Alpine to Debian Bullseye, [d8fa2c07](https://github.com/mrjackwills/adsbdb/commit/d8fa2c0747dee8e741137a7c711cf3b2a073890f), [e5b062d8](https://github.com/mrjackwills/adsbdb/commit/e5b062d837f56db5f01b8a60e5dea04849e89b1a),
+ log to file as json, [47b769e1](https://github.com/mrjackwills/adsbdb/commit/47b769e1959b973957dea89cda2eed5b97487de4), [669d6dc4](https://github.com/mrjackwills/adsbdb/commit/669d6dc4d4251086397f3c2cdc1dced0fd95749c),

### Fixes
+ impl From\<ModelAircraft> for ResponseAircraft, [6079d32d](https://github.com/mrjackwills/adsbdb/commit/6079d32de505b63b04fa0d9bf0adb1c2ab702dfc),
+ untrack Cargo.lock, [9bf63802](https://github.com/mrjackwills/adsbdb/commit/9bf63802cefcb9c903e836fd279a0e8c81fe9d62),

# <a href='https://github.com/mrjackwills/adsbdb/releases/tag/v0.0.13'>v0.0.13</a>
### 2022-08-04

### Chores
+ aggressive linting: nursery, pedantic, and unused_unwraps, [b6716880](https://github.com/mrjackwills/adsbdb/commit/b6716880fc7e5c5b168d9aeafc9288ef9dc542c8), [7a87c386](https://github.com/mrjackwills/adsbdb/commit/7a87c3864702ae7a2cbb2e7b5fc7602cd11df6fd), [13e30b0a](https://github.com/mrjackwills/adsbdb/commit/13e30b0a0179c47f14474f824fcdb78612495479),
+ dependencies updated, [ce1ef872](https://github.com/mrjackwills/adsbdb/commit/ce1ef872fe81501dc473b71def7710bd2141fdbd),

# <a href='https://github.com/mrjackwills/adsbdb/releases/tag/v0.0.12'>v0.0.12</a>
### 2022-07-21

### Chores
+ dependencies updated, [81334ac9](https://github.com/mrjackwills/adsbdb/commit/81334ac97569d011613e81a366aa42eb28efc0fc),

### Features
+ parse server bind_address from env.host_name & env.port, [1f66fb84](https://github.com/mrjackwills/adsbdb/commit/1f66fb84a4825d2ba9e17a9e16fdded0f33ebdc0),
+ parse env from file, closes [#4](https://github.com/mrjackwills/adsbdb/issues/4), [e3d0429f](https://github.com/mrjackwills/adsbdb/commit/e3d0429fe359cfbf5050c090835b97e5cb0ce588),
+ log to file and stdout, [c709446d](https://github.com/mrjackwills/adsbdb/commit/c709446db67d31ce4235cb226ff920a532caa329),
+ api.dev/Dockerfile added, [97f3fe03](https://github.com/mrjackwills/adsbdb/commit/97f3fe03db9f1c5c0b579cc68b1b63035f987e64),
+ redis conf create using .env, [a4c30f3b](https://github.com/mrjackwills/adsbdb/commit/a4c30f3ba96cb3d5efcdc57abf7af743b723f743),

### Fixes
+ Rate limit only set ttl if limit has been hit, or no key exists, [acb51fee](https://github.com/mrjackwills/adsbdb/commit/acb51fee4de826f11cfa59d37f385a426c4b5ccd),
+ change docker mount locations of databases & logs, [710066a2](https://github.com/mrjackwills/adsbdb/commit/710066a250dd364c8418395b121fa5d7767ce0b8),

### Refactors
+ run.sh & create_release.sh updated, [8855e03e](https://github.com/mrjackwills/adsbdb/commit/8855e03e384882606c00a2e4b3f028e13f1d2f83),

# <a href='https://github.com/mrjackwills/adsbdb/releases/tag/v0.0.11'>v0.0.11</a>
### 2022-06-17

### Chores
+ Update sqlx to v0.6.0, [ddf6932b](https://github.com/mrjackwills/adsbdb/commit/ddf6932b67c482c13ce441e5cf47576dafb7fb4c),

### Features
+ Use tower_http for body_limit checks, deals with edge cases better than axum extractor, [fe30bcd0](https://github.com/mrjackwills/adsbdb/commit/fe30bcd0443856d43a12c7bd396cccd91611eac9),

# <a href='https://github.com/mrjackwills/adsbdb/releases/tag/v0.0.10'>v0.0.10</a>
### 2022-06-16

### Docs
+ Add twitter links to Readme.md & site, [904259a2](https://github.com/mrjackwills/adsbdb/commit/904259a21018e34e5b73696758af574a2e17f768),

### Features
+ Return Result<(), AppError> in main(), [8ca3ad3e](https://github.com/mrjackwills/adsbdb/commit/8ca3ad3e3ffb0dc8dd4962042c3989902b7dbb68),
+ Use match in ratelimit middleware, to remove unwrap(), [2ea55d12](https://github.com/mrjackwills/adsbdb/commit/2ea55d12d733be51d210cdceb2a8ef82d1a7bea6),
+ Add connection timeouts to postgres & redis, [5e242efa](https://github.com/mrjackwills/adsbdb/commit/5e242efa7883329efe95b7659a4cf81a63e3a8d3),
+ create_release.sh update api.Dockerfile to download latest build from github, [9f29591d](https://github.com/mrjackwills/adsbdb/commit/9f29591d0018a499a982e380738bdff676dff456),

### Fixes
+ Remove ModelAircraft get unwrap, [2a28831c](https://github.com/mrjackwills/adsbdb/commit/2a28831c58bf1b02915e9b6b49ff330864778a67),

### Refactors
+ Route handlers renamed from method_routename to routename_method, [aaba380c](https://github.com/mrjackwills/adsbdb/commit/aaba380cb217ba5ee3ca0142f1c4131eeb0a2692),
+ unused code removed, [879afd0c](https://github.com/mrjackwills/adsbdb/commit/879afd0c2ff6ce4bf4b42c2b0c2a793221ddfd02),

### Tests
+ Use a TestSetup struct to contain test data, impl finish() to close db connection at end of test, [693bb60c](https://github.com/mrjackwills/adsbdb/commit/693bb60c66fdf7d53e839d4f875d552d44b6ff76),


# <a href='https://github.com/mrjackwills/adsbdb/releases/tag/v0.0.9'>v0.0.9</a>
### 2022-06-13

### Chores
+ Cargo.lock dependencies updated, [630d810c](https://github.com/mrjackwills/adsbdb/commit/630d810c8f6678fd31dab2f0a5f76647b4e84e8a),

### Fixes
+ create_release.sh sed fix, removed hard-coded value, [d3dac1b9](https://github.com/mrjackwills/adsbdb/commit/d3dac1b94a576d8aa5d4ab704f04fb4f9e2a1f53),

### Refactors
+ Change to flightroute response, use origin, destination, and occasionally midpoint, keys, all which contain an Airport Value, [2f52c9fc](https://github.com/mrjackwills/adsbdb/commit/2f52c9fcd010c51ae66521ec354ee333563a7f61),
+ Api Dockerfile download binary from github instead of building, [d69ad32a](https://github.com/mrjackwills/adsbdb/commit/d69ad32a585be102959a38e5b3196817e0123a3b),
+ github workflow renamed, [5e7ee6fe](https://github.com/mrjackwills/adsbdb/commit/5e7ee6fe923aa886f894e4a765391709cc8e34d5),

# <a href='https://github.com/mrjackwills/adsbdb/releases/tag/v0.0.8'>v0.0.8</a>
### 2022-06-12

### Chores
+ Cargo.toml update dependencies, [23e6e0c8](https://github.com/mrjackwills/adsbdb/commit/23e6e0c8abcca091e6a62d1795e4a645faeec96f),

### Docs
+ readme updated, [126e544e](https://github.com/mrjackwills/adsbdb/commit/126e544e24f151bbd07824a65100a24454f60198), [48d4c2d4](https://github.com/mrjackwills/adsbdb/commit/48d4c2d42f80c798b89ec889c8a8ed7fbed150e4),

### Features
+ N-Number to Mode-S conversion, also include n_number in aircraft response, closes [#1](https://github.com/mrjackwills/adsbdb/issues/1), [2f0b9052](https://github.com/mrjackwills/adsbdb/commit/2f0b9052e1121022183da34bdd05e7d76e402a83),
+ Use ResponseAircraft/ResponseFlightRoute to return data to user, [ca1cd114](https://github.com/mrjackwills/adsbdb/commit/ca1cd114bbcbdeaf9623f43b9c2d6a8a2eca68c3),

### Fixes
+ use sqlx fetch_optional, [41122b83](https://github.com/mrjackwills/adsbdb/commit/41122b83a366f596b31df062776482fb31fc5f25),
+ init.db, elevation as INT, [efbdb34f](https://github.com/mrjackwills/adsbdb/commit/efbdb34f89561f6f6fed29a27a8b9c3bc9babae2),

### Refactors
+ Dead code removed, [3816f3b9](https://github.com/mrjackwills/adsbdb/commit/3816f3b9edbedf8f863cd8911eb894760eacff62),

# <a href='https://github.com/mrjackwills/adsbdb/releases/tag/v0.0.7'>v0.0.7</a>
### 2022-05-31

### Chores
+ docker containers bump alpine to 3.16, [d6d65f62](https://github.com/mrjackwills/adsbdb/commit/d6d65f6274a242fd5498a278673423e7745e8e61),

### Docs
+ Readme improved, [21b4c6e9](https://github.com/mrjackwills/adsbdb/commit/21b4c6e915cc8ceb6ec6b63232eff27b1e006013),

### Features
+ .github workflow build for x86_64 musl, [ed468442](https://github.com/mrjackwills/adsbdb/commit/ed468442b510656c64e1ce2c20c0b5a0c0dfb940),

### Fixes
+ website example responses fixed typo, [a4038a2b](https://github.com/mrjackwills/adsbdb/commit/a4038a2b2f3169f61aae288d83401b3c365c5369),

# <a href='https://github.com/mrjackwills/adsbdb/releases/tag/v0.0.6'>v0.0.6</a>
### 2022-05-10

### Chores
+ update dependacies, [af54e606](https://github.com/mrjackwills/adsbdb/commit/af54e606a91e6a46a578971c988009453a60a3da), 

### Features
+ set content-body to max length of 1024, [269f5a7f](https://github.com/mrjackwills/adsbdb/commit/269f5a7f3a2db1d0ba6e82085045f73fd7984e93),

# <a href='https://github.com/mrjackwills/adsbdb/releases/tag/v0.0.5'>v0.0.5</a>
### 2022-05-10

### Features
+ Added a DATA github issue template, and link to it on the site, [c939b89b](https://github.com/mrjackwills/adsbdb/commit/c939b89b2df2db4413bf474ee52a3dd8e9b1de3b),

# <a href='https://github.com/mrjackwills/adsbdb/releases/tag/v0.0.4'>v0.0.4</a>
### 2022-05-09

### Features
+ Basic website added, [ef0ddadf](https://github.com/mrjackwills/adsbdb/commit/ef0ddadf1d1610a2ae7ba481a28fbcb497faba44),
+ Update website version number on create release, [d91dc82c](https://github.com/mrjackwills/adsbdb/commit/d91dc82c2a78b23ba99199d0d20ff0110496efce),

# <a href='https://github.com/mrjackwills/adsbdb/releases/tag/v0.0.3'>v0.0.3</a>
### 2022-05-09

### Fixes
+ Docker init restore data from pg_dump fixed, api health_check fixed, [1927d0d9](https://github.com/mrjackwills/adsbdb/commit/1927d0d9324577304e400d1c6c6ff1eeb3ba8467),

# <a href='https://github.com/mrjackwills/adsbdb/releases/tag/v0.0.2'>v0.0.2</a>
### 2022-05-09

### Features
+ Init commit
