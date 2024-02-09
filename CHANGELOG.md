### Chores
+ create_release v0.5.2, [9915bb745e39b6fc3d51f2875677d8bb284f96d5]
+ run.sh v0.2.0, [960854b2119ef8d264facb5a259de5202bcb148a]
+ .devcontainer updated, [51bf7c33859a8bac38579f5208391e07ae7175c6], [a5407118d6e45afd6165a4a8ec2858c04d436e96]
+ dependencies updated, [b16951bdfb566a3120899e66ecb7f1a3e3003bda], [e1254ad31e431cd2485871c58e8e7c2314f4ac77], [dc9db787e0731d5c7db66b9a512df3f6cad5a1dd], [1427c4722173e5cece9b18a39229c93198166698]

### Features
+ change redis client to Fred, [df40d99430fbfc9a8610f56496bc23ab57064365]
+ matrix cross platform GitHub build, [eb7daedf0197d09771f5a5814805ce9c7338ca22]

### Fixes
+ create_release sqlx prepare, [65d23c4f203f72f9c0e5c8d67bad171c007b071e]
+ rate limit, closes #5, [7eb709ad18511197ff4c534b88d68e1e48b59ca1]

### Refactors
+ sql files linted, [e51cfceb3d11ba7d5f1e00865ddde92109c649d3]
+ DockerFiles cleaned up, [77dc017b9c490e13e3fd0a1254f40c307c09aa13]

# <a href='https://github.com/mrjackwills/adsbdb/releases/tag/v0.2.7'>v0.2.7</a>
### 2024-01-05

### Chores
+ .devcontainer updated, [9f65dd57](https://github.com/mrjackwills/adsbdb/commit/9f65dd57d285488ec0454dfdc979f09c2e9edc8c), [a6553e96](https://github.com/mrjackwills/adsbdb/commit/a6553e96e19845498df86d91ea98db006775b264)
+ Alpine version bump, [218e6e5a](https://github.com/mrjackwills/adsbdb/commit/218e6e5ac48436252d27222d1c1983cb0a027de7)
+ dependencies updated, [4f6bb126](https://github.com/mrjackwills/adsbdb/commit/4f6bb1266eaa4c8717e94d64d6c4705d83adcb07), [b242e2d4](https://github.com/mrjackwills/adsbdb/commit/b242e2d4ef23d72245d1e09b38790727ec3a09bd), [ca177102](https://github.com/mrjackwills/adsbdb/commit/ca17710265bb45714655cfee8c6df631ca6d994d)
+ file linting, [815cde77](https://github.com/mrjackwills/adsbdb/commit/815cde77e5bbc56421e3cf3bb7a63a9c0f8c6882)
+ Rust 1.75.0 linting, [83a68787](https://github.com/mrjackwills/adsbdb/commit/83a687872c3069cd1364aadbf88f3b9b8d45d57f)

### Features
+ run.sh v0.1.0, [fbdd1a94](https://github.com/mrjackwills/adsbdb/commit/fbdd1a9423772b27a53d1d35c5b479f180dd3818)

### Fixes
+ ApiError import/export,[0400ab66f8f877658830b57d85664415f4b649b3]
+ .gitattributes(?), [b238df27](https://github.com/mrjackwills/adsbdb/commit/b238df27d297b11254ce5a419225cae89c9c307d)
+ redis dependency updates, [e6da9e0a](https://github.com/mrjackwills/adsbdb/commit/e6da9e0a0a60862ee0eaba3f929da320792bcd82)
+ scraping error message more detail, [a3a1263f](https://github.com/mrjackwills/adsbdb/commit/a3a1263f32a9afded8fc27d93779823d91549dd2)

### Refactors
+ dead code removed & re-ordered, [9d6d51bb](https://github.com/mrjackwills/adsbdb/commit/9d6d51bb5473d1752dd24b516772caca968f0d6a)

### Reverts
+ graceful shutdown re-introduced, [2fa3c1cc](https://github.com/mrjackwills/adsbdb/commit/2fa3c1cc95b553197954a989e3e6f7a26115c90e)

### Tests
+ ratelimit tests improvement, [cbd7bb78](https://github.com/mrjackwills/adsbdb/commit/cbd7bb78b24711cf4c004b4679ed9d9599ca38fd)

# <a href='https://github.com/mrjackwills/adsbdb/releases/tag/v0.2.6'>v0.2.6</a>
### 2023-11-28

### Chores
+ dependencies update, lints into Cargo.toml, [ce5b724a](https://github.com/mrjackwills/adsbdb/commit/ce5b724aa2f2623c3503515eaac8e5e3757e4713), [26688650](https://github.com/mrjackwills/adsbdb/commit/266886505783ab6ceec1a2ac1dd7b5aecdb588ec)
+ update PostgreSQL to v16, [bd409cde](https://github.com/mrjackwills/adsbdb/commit/bd409cde79e78e068e1f15a73ae04ec57b16b8bb), [1519d731](https://github.com/mrjackwills/adsbdb/commit/1519d7319ec7c2eed3fece726507a69311747e76)
+ update axum to 0.7, [a9772d25](https://github.com/mrjackwills/adsbdb/commit/a9772d253b77f9b687b207afda85f5306158cd1e), [c4232dd6](https://github.com/mrjackwills/adsbdb/commit/c4232dd640d67682b5d332854752b78b6a3ea75b)
+ .devcontainer updated, [101da4d3](https://github.com/mrjackwills/adsbdb/commit/101da4d3fc8829b49d339091840a30099ce17e7d), [abcc5d0b](https://github.com/mrjackwills/adsbdb/commit/abcc5d0bc5ece7865d46cc41d6999212083bb8ab), [2cb95992](https://github.com/mrjackwills/adsbdb/commit/2cb9599205f1852b7203b285e0faadc5f42bba02)
+ Rust 1.73 linting, [f84617fb](https://github.com/mrjackwills/adsbdb/commit/f84617fb22d97dc44966f97cfbfc84ef85036ba2)
+ dependencies updated, [c7ab1ecb](https://github.com/mrjackwills/adsbdb/commit/c7ab1ecb0e54dc077bf1999e505a5625efe70fb7), [3c714e59](https://github.com/mrjackwills/adsbdb/commit/3c714e59f80cc01a2917bdc7b605c417736a716d)
+ adsbdb.com site updated, [5e6b74af](https://github.com/mrjackwills/adsbdb/commit/5e6b74af34e4a3adfffb7ab7c66c1ca251bc6146)

### Features
+ ApplicationState placed into an Arc, [47287de8](https://github.com/mrjackwills/adsbdb/commit/47287de8703e0bb9386a90dd7d9f7d82bb05f208)
+ Track scrapers in a hashmap, [88cca3b0](https://github.com/mrjackwills/adsbdb/commit/88cca3b0274b4b53d8cd63d130c3dc34994d4437)
+ ModelAirline & ModelFlightroue use PgPool instead of transaction, [ddf06c08](https://github.com/mrjackwills/adsbdb/commit/ddf06c08427fd4184af94fdf27842e8ea914d8dc)
+ &String -> &str, [c3eef0d9](https://github.com/mrjackwills/adsbdb/commit/c3eef0d9236472b240e03900dd5005e1a66fd2ac)

### Fixes
+ ModelFlightRoute function call, [980fde0d](https://github.com/mrjackwills/adsbdb/commit/980fde0ddbe1fbaa829aebcd7aaed5350d16a82f)
+ ratelimit attempted fix, [51fdf569](https://github.com/mrjackwills/adsbdb/commit/51fdf56994ae0288ccad2d532397ae5654aee507)

### Refactors
+ redis_to_serde tracing, [0a520636](https://github.com/mrjackwills/adsbdb/commit/0a52063688e02fc7ff477718fed83c1eaff53e3f)

# <a href='https://github.com/mrjackwills/adsbdb/releases/tag/v0.2.5'>v0.2.5</a>
### 2023-08-26

### Chores
+ Rust 1.72.0 linting, [459d3c56](https://github.com/mrjackwills/adsbdb/commit/459d3c5632496c954622107f6f1845e4da9545a0)
+ dev container psql install, .docker/config delete, [b9dc0b89](https://github.com/mrjackwills/adsbdb/commit/b9dc0b893f5054d0c5313de75eaf5553ef6ccada), [9c32aaf4](https://github.com/mrjackwills/adsbdb/commit/9c32aaf419f76917c9d8050252e0519f716e4695)
+ dependencies updated, [9773f097](https://github.com/mrjackwills/adsbdb/commit/9773f097097b47636d697ef44941a8c43c034d61), [05f522fb](https://github.com/mrjackwills/adsbdb/commit/05f522fbf1b5fdedabb25a8e8e32d968ec635cde)

### Fixes
+ correctly copy .sqlx files into container, [6e76d617](https://github.com/mrjackwills/adsbdb/commit/6e76d6176336ba69527b0dbb146b8ec3711f699d)

### Test
+ scraped transaction callsign change, [22c67fc7](https://github.com/mrjackwills/adsbdb/commit/22c67fc71e08548fe3929214bfc67fb09825c19b)

# <a href='https://github.com/mrjackwills/adsbdb/releases/tag/v0.2.4'>v0.2.4</a>
### 2023-07-29

### Chores
+ create_release 0.3.0, [0942f4b0](https://github.com/mrjackwills/adsbdb/commit/0942f4b0fe3fa83a7b0f6b70476f61f571b508cf)
+ dependencies updated, [65a1ce2d](https://github.com/mrjackwills/adsbdb/commit/65a1ce2d4d024e1292bc627b80d38ed1bf28f61f)

### Features
+ rate limits increased, lower: 120 -> 512, upper: 240 -> 1024, [49af6b29](https://github.com/mrjackwills/adsbdb/commit/49af6b2970f88e0b097e90e76aa4598bab0e0df8)
+ sqlx logging based on env level, [e7d25648](https://github.com/mrjackwills/adsbdb/commit/e7d256488d78670bd999e5be0fdadd859e4912c1)

### Fixes
+ copy sqlx prepared files into api.Dockerfile, [9ed3944c](https://github.com/mrjackwills/adsbdb/commit/9ed3944cd8a35e65b86696844db079d562b77669)

# <a href='https://github.com/mrjackwills/adsbdb/releases/tag/v0.2.3'>v0.2.3</a>
### 2023-06-11

### Chores
+ dependencies updated, [109c7a72](https://github.com/mrjackwills/adsbdb/commit/109c7a72ef21652b3269fd3a86c0d5842c3ebf70), [66eee54e](https://github.com/mrjackwills/adsbdb/commit/66eee54ede84af8cd0a5e18ae9c51186ebb3a724), [88fd8d74](https://github.com/mrjackwills/adsbdb/commit/88fd8d7447d67c7eae879722c4669cf1032240f9), [c827db21](https://github.com/mrjackwills/adsbdb/commit/c827db21e6375e964e9e39dfa48fced6964bdc27), [e94098e4](https://github.com/mrjackwills/adsbdb/commit/e94098e48cd5bafbf8b4fef93b06d52a65f4033e), [fcec134f](https://github.com/mrjackwills/adsbdb/commit/fcec134f6e05ddee4accfd7ead4e2c608e646c00)
+ Dockerfiles Alpine bump to 3.18, [4f55f7c6](https://github.com/mrjackwills/adsbdb/commit/4f55f7c63ce0fec02aacc6b18bcfc0a80bec2f2b)
+ sqlx prepare, [6057dce2](https://github.com/mrjackwills/adsbdb/commit/6057dce2c6a9fce7b5d3df72f907a1cd4d95f2fa), [e8139a9d](https://github.com/mrjackwills/adsbdb/commit/e8139a9da2d344211a73e8cdf46703c453d940c4), [e355807c](https://github.com/mrjackwills/adsbdb/commit/e355807c706cc807c61d82c8ddeb14039911d74f)

### Docs
+ CONTRIBUTING.md added, [676d06f6](https://github.com/mrjackwills/adsbdb/commit/676d06f6a7a1766b58d1b2bbd9f240afc0726956)
+ GitHub data issue add source, [4ae44c25](https://github.com/mrjackwills/adsbdb/commit/4ae44c25716a29eb81e02c2142e4ac5fffa87f29)

### Features
+ `define_routes` macro, [8daf8585](https://github.com/mrjackwills/adsbdb/commit/8daf85854e49784294580f961ae0b9ae47444d44)
+ `sleep` macro, [21f1b81a](https://github.com/mrjackwills/adsbdb/commit/21f1b81a2a02cbc8a170c52d4a7b3961ec067642)
+ `unit_struct` & `from_request_parts` macros, [61e2391c](https://github.com/mrjackwills/adsbdb/commit/61e2391c59ed36870b8aaa2640002db6b2024bc2), [6e52dc26](https://github.com/mrjackwills/adsbdb/commit/6e52dc267bb06fc96341d73662ccff6dfb8a445e)
+ app_error internal! macro, [c3f49717](https://github.com/mrjackwills/adsbdb/commit/c3f497171b387c36a26429e100c0e2113124fd3a)
+ create_release prepare sqlx, [f05b6b15](https://github.com/mrjackwills/adsbdb/commit/f05b6b15ee87c4e391bbf9758361d495e69415af)
+ devcontainer install sqlx-cli, [1403434d](https://github.com/mrjackwills/adsbdb/commit/1403434d1f0e36452ba9f3f4d8b6923b06eb5225)
+ from_request_parts macro, [c5a5e2e6](https://github.com/mrjackwills/adsbdb/commit/c5a5e2e6c49b4f40510099ae6a4815ce9e46f41b)
+ ModelAirport use macro, [c9d01503](https://github.com/mrjackwills/adsbdb/commit/c9d01503c5d18a20eb25cd7cf45b97cf3f128e68)
+ ModelAircraft use macros, [ab72f2eb](https://github.com/mrjackwills/adsbdb/commit/ab72f2eba49d768f9ef998cadd12cb1f91b895e6)
+ ModelAirline use macro, [6619fe9b](https://github.com/mrjackwills/adsbdb/commit/6619fe9b4638d5d81e35a105092e1780b371164f)
+ ModelAirport query improved, [b9d603c8](https://github.com/mrjackwills/adsbdb/commit/b9d603c85d856cb2b80cf02283e677872e59224a)
+ ModelFlightroute, use JOINS rather than subqueries, [3d31ec2a](https://github.com/mrjackwills/adsbdb/commit/3d31ec2a7bef3398d6b35352c3a43a09dc01bf84)

### Fixes
+ GitHub workflow use regex for semver tag, [de759047](https://github.com/mrjackwills/adsbdb/commit/de75904774749346af87b73e5eae0234c61b368a)
+ nursery redis drop lint, [535f1eee](https://github.com/mrjackwills/adsbdb/commit/535f1eee02bca5df9387803fc7bb64eede04e630)

### Refactors
+ dead code removed, [91a4a404](https://github.com/mrjackwills/adsbdb/commit/91a4a4043c0da7d0c68c132e6442e144aa30daf8)
+ methods renamed, [16337e76](https://github.com/mrjackwills/adsbdb/commit/16337e76f140731c953f35fa71239fcb740803ff), [23686cec](https://github.com/mrjackwills/adsbdb/commit/23686cecbe8eef8a6e702176098ee7faf187cf98)
+ ModelFlightRoute query refactors, and macros, [1c296a1b](https://github.com/mrjackwills/adsbdb/commit/1c296a1b7fed5c9e83bf53501948c4a9ccd1f12f)
+ mods moved, [03606946](https://github.com/mrjackwills/adsbdb/commit/03606946cd0ad159a12dc40692df0d504c4ff4aa)
+ ModelFlightroute sql query into parts, [cb78968b](https://github.com/mrjackwills/adsbdb/commit/cb78968befa521b1589ba4635c89e1c2e9db84ad)
+ ModelFlightroute insert scraped flightroute use macros, [b64c3454](https://github.com/mrjackwills/adsbdb/commit/b64c34549e87f180f399f757a77edb05d4fe3659)

### Reverts
+ .devcontainer sparse protocol now default, [430ce92b](https://github.com/mrjackwills/adsbdb/commit/430ce92bebbc7e856612f80fcef754b507f2d426)

# <a href='https://github.com/mrjackwills/adsbdb/releases/tag/v0.2.2'>v0.2.2</a>
### 2023-03-13

### Chores
+ dependencies updated, [ac52eb8d](https://github.com/mrjackwills/adsbdb/commit/ac52eb8deb75c18a04ac13a8ba216b4df6ea84d8)
+ devcontainer use sparse protocol index, [9e167c5f](https://github.com/mrjackwills/adsbdb/commit/9e167c5fc830e0f2be312c027a5b235c73cb59e3)
+ Rust 1.68.0 linting, [13352ff0](https://github.com/mrjackwills/adsbdb/commit/13352ff02bf9cab87c50c31f072c4a928f455120)

### Fixes
+ LIMIT 1 in sql queries, [9caa5824](https://github.com/mrjackwills/adsbdb/commit/9caa5824015616f954fff8d9ab6120b0d78cfed7)

# <a href='https://github.com/mrjackwills/adsbdb/releases/tag/v0.2.1'>v0.2.1</a>
### 2023-03-05

### Chores
+ dependencies updated, [765c1c7b](https://github.com/mrjackwills/adsbdb/commit/765c1c7b91da0f83ef5e739ef8fbb34047e71730), [fccf173d](https://github.com/mrjackwills/adsbdb/commit/fccf173daedf96d787a0d99decc9667a3485404f)

### Docs
+ readme updated, [1cb02ed8](https://github.com/mrjackwills/adsbdb/commit/1cb02ed8aec427ec649c3a94ab6b3d82ed0a7b4e)

### Features
+ flightroute::_get() now just return an Option, [6fa60c20](https://github.com/mrjackwills/adsbdb/commit/6fa60c204c29e1dd5c32b21b818ec139f75a74dc)
+ `_typos.toml` added, [ddb63313](https://github.com/mrjackwills/adsbdb/commit/ddb633130ed63e8bb4be388d809f91821028ca45)

### Fixes
+ postgres.Dockerfile typo, [6da6fd4f](https://github.com/mrjackwills/adsbdb/commit/6da6fd4f40200f2a47206831711f7ec2623b5463), [944c5312](https://github.com/mrjackwills/adsbdb/commit/944c53124a05c389b424090ed2654bba6c287a56)

### Refactors
+ postgreSQL queries use `USING(x)` where appropriate, [5c8adf00](https://github.com/mrjackwills/adsbdb/commit/5c8adf0049b322b1198b9eee55a09c0d3f592fdd), [686a1783](https://github.com/mrjackwills/adsbdb/commit/686a1783292de9875a4c025915bf5958b154aaa3), [9515a416](https://github.com/mrjackwills/adsbdb/commit/9515a4167f4aab84872ae758bbe4b403ef8f7ba7)

### Reverts
+ temporary devcontainer buildkit fix removed, [6c07fa67](https://github.com/mrjackwills/adsbdb/commit/6c07fa675952f2a77a5f570f0ebe07b9c42f9bbb)

# <a href='https://github.com/mrjackwills/adsbdb/releases/tag/v0.2.0'>v0.2.0</a>
### 2023-02-25

**This release has, potential, Breaking Changes**

### Chores
+ dev container updated, [1ac83bdb](https://github.com/mrjackwills/adsbdb/commit/1ac83bdb561145e101b6b3bc2c27c35471b25b50), [a398c8cc](https://github.com/mrjackwills/adsbdb/commit/a398c8cc09ce4f37520137ae8f91087d55f36efd)
+ create_release updated, [eb8f871d](https://github.com/mrjackwills/adsbdb/commit/eb8f871deba42e035595918c0c492e2ca4f0d156), [ea93d0b6](https://github.com/mrjackwills/adsbdb/commit/ea93d0b6c585d7fe5f0d050822631ad8cad46cb1)
+ dependencies updated, [01204465](https://github.com/mrjackwills/adsbdb/commit/01204465e1a36bbb15cf4d37cdf44398e394449c), [87c9c0e6](https://github.com/mrjackwills/adsbdb/commit/87c9c0e63e2e86027a07b44e031b0e1614950cdb), [a8d138e0](https://github.com/mrjackwills/adsbdb/commit/a8d138e0f2775e96fa4dc6516fa905e3b007446a), [5322f1de](https://github.com/mrjackwills/adsbdb/commit/5322f1de46881984003a83d7d2063ea0172cb3da), [6e83e199](https://github.com/mrjackwills/adsbdb/commit/6e83e199ef4e99773b9d4790c11ff4098fb3abb9), [a4821b9a](https://github.com/mrjackwills/adsbdb/commit/a4821b9ac28c2e563916e40c18aac8900bfc35c9)

### Docs
+ site updated, [3c4bcb49](https://github.com/mrjackwills/adsbdb/commit/3c4bcb49e6f0d23cc7377fcecf399f74d8067b66)
+ various comment typos, [1af07db8](https://github.com/mrjackwills/adsbdb/commit/1af07db8faaeeda55a45b386cebd851193ace79e)
+ site uptime automatically increase, [678bb062](https://github.com/mrjackwills/adsbdb/commit/678bb062895215f6b8de3dcf6bd5e585a3a8db3a)

### Features
**Breaking Changes**
+ Callsigns & Flightroutes are now stored much more efficiently in the database, split, if possible, by IATA/ICAO prefix, and a suffix. This means that when searching for a Flightroute, one can use either the IATA or ICAO callsign.
The callsign response now includes a `callsign_iata` and `callsign_icao` key, as well as an Airline object (see the [README.md](https://github.com/mrjackwills/adsbdb/blob/main/README.md) or [adsbdb.com](https://www.adsbdb.com) for more information). A new `/airline` route is now available, which will search
for Airlines, again based on either `ICAO` or `IATA` airline codes, and will either return an array of Airlines, or a 404 response, [d1f614d3](https://github.com/mrjackwills/adsbdb/commit/d1f614d3b5288dc000aa026a825e6f9f14b06f54)
+ Add an env to disable flightroute & photo scraping, [1024d7f7](https://github.com/mrjackwills/adsbdb/commit/1024d7f7715f97c86a5e0ca40a906633b8f6029a)
+ Dockerfiles updated, build from source, [7c9e4861](https://github.com/mrjackwills/adsbdb/commit/7c9e4861f77191d9cca904dd3c32e8ada8bae294), [2bd3df6d](https://github.com/mrjackwills/adsbdb/commit/2bd3df6d93505cb9132a72b0524946040f56317d)
+ openssl dependency removed, [7870c7d1](https://github.com/mrjackwills/adsbdb/commit/7870c7d19c260906b1f21610a4a09dc9a5a46cad)
+ force exit if database connection error, [d950b39f](https://github.com/mrjackwills/adsbdb/commit/d950b39f0527d0419ff1219c7033ae6782d2cba3)
+ dev postgres run from /dev/shm, auto pg_dump restoration, [c5eb2466](https://github.com/mrjackwills/adsbdb/commit/c5eb2466b67fa45608c8c6356389ab5f91b4aaaf), [ad171abd](https://github.com/mrjackwills/adsbdb/commit/ad171abdb487d1db90635eea866fa11ca0edaeb6)
+ backup use age, [00c9d63d](https://github.com/mrjackwills/adsbdb/commit/00c9d63da8b891fdfb0b6651aef643a1b62ff4b8)

### Fixes
+ increase redis docker memory limit, [a58b6a7e](https://github.com/mrjackwills/adsbdb/commit/a58b6a7eaf219d2ac5c2d0becbd149b4aa1522af), [ce228249](https://github.com/mrjackwills/adsbdb/commit/ce22824918bd56b48d077506d0edffa8dfde5905)

### Refactors
+ Rust 1.67.0 clippy linting, [b3ff5c49](https://github.com/mrjackwills/adsbdb/commit/b3ff5c4965f05ba0eecdb71569dc6908296d16f6)
+ dead code removed, [427bb899](https://github.com/mrjackwills/adsbdb/commit/427bb899439b313ba3df0278f4dbc99f9d324c81)

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
