### 2023-01-14

**This release has breaking changes in the response Aircaft JSON object**

### Chores
+ dependencies updated, [680af9c7d94e2bb00b79a3e3e77f4058eeea4977], [227cb14a1aef740d818654a2dc20a85877e0cf1c]

### Debug
+ ratelimit tracing, [f68df99caf4bb533afa1daf9439e593de25a8f92]

### Features
**Breaking Change**
+ `n_number` is now `registration`, the api now returns, or attempts to return, a registration for every aircraft, closes #13. The `/aircraft/x` route now accepts either mode_s hex code or aircraft registration for `x`, [b468fa824575322e64142ed031b9de158c46fb52]

### Fixes
+ Use a reqwest::Client builder, to enable request timeout, as well as gzip & brotli, [57bd31d95501c8ae6b1bc4ca88f92035ce137450]


see <a href='https://github.com/mrjackwills/adsbdb/blob/main/CHANGELOG.md'>CHANGELOG.md</a> for more details
