### Docs
+ Add twitter links to Readme.md & site, [904259a21018e34e5b73696758af574a2e17f768]

### Features
+ Return Result<(), AppError> in main(), [8ca3ad3e3ffb0dc8dd4962042c3989902b7dbb68]
+ Use match in ratelimit middleware, to remove unwrap(), [2ea55d12d733be51d210cdceb2a8ef82d1a7bea6]
+ Add connection timeouts to postgres & redis, [5e242efa7883329efe95b7659a4cf81a63e3a8d3]
+ create_release.sh update api.Dockerfile to download latest build from github, [9f29591d0018a499a982e380738bdff676dff456]

### Fixes
+ Remove ModelAicraft get unwrap, [2a28831c58bf1b02915e9b6b49ff330864778a67]

### Refactors
+ Route handlers renamed from method_routename to routename_method, [aaba380cb217ba5ee3ca0142f1c4131eeb0a2692]
+ unused code removed, [879afd0c2ff6ce4bf4b42c2b0c2a793221ddfd02]

### Tests
+ Use a TestSetup struct to contain test data, impl finish() to close db connection at end of test, [693bb60c66fdf7d53e839d4f875d552d44b6ff76]


# <a href='https://github.com/mrjackwills/adsbdb/releases/tag/v0.0.9'>v0.0.9</a>
### 2022-06-13

### Chores
+ Cargo.lock dependancies updated, [630d810c](https://github.com/mrjackwills/adsbdb/commit/630d810c8f6678fd31dab2f0a5f76647b4e84e8a),

### Fixes
+ create_release.sh sed fix, removed hard-coded value, [d3dac1b9](https://github.com/mrjackwills/adsbdb/commit/d3dac1b94a576d8aa5d4ab704f04fb4f9e2a1f53),

### Refactors
+ Change to flightroute response, use origin, destination, and occassionally midpoint, keys, all which contain an Airport Value, [2f52c9fc](https://github.com/mrjackwills/adsbdb/commit/2f52c9fcd010c51ae66521ec354ee333563a7f61),
+ Api Dockerfile download binary from github instead of building, [d69ad32a](https://github.com/mrjackwills/adsbdb/commit/d69ad32a585be102959a38e5b3196817e0123a3b),
+ github workflow renamed, [5e7ee6fe](https://github.com/mrjackwills/adsbdb/commit/5e7ee6fe923aa886f894e4a765391709cc8e34d5),

# <a href='https://github.com/mrjackwills/adsbdb/releases/tag/v0.0.8'>v0.0.8</a>
### 2022-06-12

### Chores
+ Cargo.toml update dependencies, [23e6e0c8](https://github.com/mrjackwills/adsbdb/commit/23e6e0c8abcca091e6a62d1795e4a645faeec96f),

### Docs
+ readme updated, [126e544e](https://github.com/mrjackwills/adsbdb/commit/126e544e24f151bbd07824a65100a24454f60198),, [48d4c2d4](https://github.com/mrjackwills/adsbdb/commit/48d4c2d42f80c798b89ec889c8a8ed7fbed150e4),

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
