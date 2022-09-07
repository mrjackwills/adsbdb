### 2022-09-07

### Chores
+ Dependencies - tokio updated, anyhow removed, [83c361d0e86a94108480b7b22b940d9f631d69f8]
+ dev container updated, from buster to bullseye, [8ec1b8e9d47de8a501905a17717d97ffef26859a]

### Features
+ store cache values in using Redis Hashes, [42871bec7941178467cedc44923dc79ba783a391]
+ website response explanation added Midpoint, [7639eec2dabde108160b19649f4f3ae040171c25]

### Fixes
+ postgres queries use uppercase text for SQL lang, [8ad0d341e7e26d7b0e6ad0ad9205146ba9fc7c1e]
+ N_Number functions replace unwrwaps() with map_or_else, [aabcdda0f5eeac2478c0b2bc372d8bd8e5143860]

### Refactors
+ ModelAircraft use &self, instead of &Self, as param, [4c39177852ca9d82e492cbd26bf7c6ce4a4c1669]
+ Redis, key.to_string() once, and optional_null turbofish matching [85bbb6bfa423bde8ec865d8c745e1a1a014f4b1d]


see <a href='https://github.com/mrjackwills/adsbdb/blob/main/CHANGELOG.md'>CHANGELOG.md</a> for more details
