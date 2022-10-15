### 2022-10-15

### Chores
+ create_release.sh v0.1.1, [065daa3d6a4efa28a75bb7fe97ed2c94c426966d]

### Docs
+ readme updated, [94e1ea83d36802876f072695065aac5df02f2c38]
+ comment typos fixed, [9d799ca37aa968a228efe77667c6e0084d1305f0]
+ comments updated, [b1ccd36e6615922a8f6bf543a882fbf21f510195]

### Features
+ NNumber, ModeS & Callsign new_types, [6a42752e3395a837fb1abc42e73768d5ec2b583c]
+ Exit with code 1 if no .env file found, [0939a2d38ffd633a029eee6e95c21062249e3e45]
+ UnknownAC (Aircraft/Callsign) enum, [596cfa07ac0704aaa3f8efd7d07d029e2a732c0c]

### Fixes
+ ttl as isize instead of usize, [84dbbf02d66bfbf6b529f82a694ea4bb3946d419]
+ Cargo.lock tracked, [366a90eeb52122ec46b18395d2aac83363178be7]
+ cache aircraft/flightroute with Cache enum, [5118d3b42a043e72a02a9f31daaf7ca7608f4b5c]
+ SQL query uppercase SQL reserved words, [e918e88c022d3d8b903156ccc82a03387edc07d5]
+ website added Aircraft Value table, [5ef5e1c7063eba42a3728ef2fcf94de42cd93258]
+ try/catch on website script, [94be3918d43719431960c2479fa0df57adec2743]
+ print version number when starting, [19de8a9d2c8a4c3ae2dd0b8cb2e01722130e5f00]

### Refactors
+ get_addr() for creating axum usable address from &env fix: get_api_version use spit instead of chars, [e6657a0a2225c0a1eb4ce4bacaa65b6e7d96eac0]
+ photo_scraper return Option<T> instead of Result<Option<T>, ApiError>, [842adfc90212431a0aedf7368520909264d21a65]
+ dead code removed, [97e57f116349beef4cd62a5daf67f6bcb6cd4753], [321db5d9ac5cc2b4ffde04a69bd9d5ea3ce3cbc2]
+ sql query syntax tweaked, [40e5b5d0fd403a4d60edaff4ef1648cb4878ad10]
+ AppError into own module, [fbc6be1428544c4b7e0425273a35640c205ab32d]


see <a href='https://github.com/mrjackwills/adsbdb/blob/main/CHANGELOG.md'>CHANGELOG.md</a> for more details
