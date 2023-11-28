### 2023-11-28

### Chores
+ dependencies update, lints into Cargo.toml, [ce5b724aa2f2623c3503515eaac8e5e3757e4713], [266886505783ab6ceec1a2ac1dd7b5aecdb588ec]
+ update PostgreSQL to v16, [bd409cde79e78e068e1f15a73ae04ec57b16b8bb], [1519d7319ec7c2eed3fece726507a69311747e76]
+ update axum to 0.7, [a9772d253b77f9b687b207afda85f5306158cd1e], [c4232dd640d67682b5d332854752b78b6a3ea75b]
+ .devcontainer updated, [101da4d3fc8829b49d339091840a30099ce17e7d], [abcc5d0bc5ece7865d46cc41d6999212083bb8ab], [2cb9599205f1852b7203b285e0faadc5f42bba02]
+ Rust 1.73 linting, [f84617fb22d97dc44966f97cfbfc84ef85036ba2]
+ dependencies updated, [c7ab1ecb0e54dc077bf1999e505a5625efe70fb7], [3c714e59f80cc01a2917bdc7b605c417736a716d]
+ adsbdb.com site updated, [5e6b74af34e4a3adfffb7ab7c66c1ca251bc6146]

### Features
+ ApplicationState placed into an Arc, [47287de8703e0bb9386a90dd7d9f7d82bb05f208]
+ Track scrapers in a hashmap, [88cca3b0274b4b53d8cd63d130c3dc34994d4437]
+ ModelAirline & ModelFlightroue use PgPool instead of transaction, [ddf06c08427fd4184af94fdf27842e8ea914d8dc]
+ &String -> &str, [c3eef0d9236472b240e03900dd5005e1a66fd2ac]

### Fixes
+ ModelFlightRoute function call, [980fde0ddbe1fbaa829aebcd7aaed5350d16a82f]
+ ratelimit attempted fix, [51fdf56994ae0288ccad2d532397ae5654aee507]

### Refactors
+ redis_to_serde tracing, [0a52063688e02fc7ff477718fed83c1eaff53e3f]

see <a href='https://github.com/mrjackwills/adsbdb/blob/main/CHANGELOG.md'>CHANGELOG.md</a> for more details
