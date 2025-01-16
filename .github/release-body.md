### 2025-01-16

### Chores
+ Docker container dependencies updated, [a49a02bd2761445c4faa053911961b9547a6358c]
+ dependencies updated, [2faab418ccc1fa2e931115b6379414f1650b10c2], [05320923e115eecfba971f34d065129e4d1d4abb]

### Docs
+ update README.md and site, [5ac06dd1bf9f7d65ca43f85e066196753748bd5a], [fc35a0fb5edc835626be0b9b0cea249dc53e0232]

### Features
+ Update POST endpoints, [646e42c23d7ae9f8c72402f53c5be8ba3e000e60]

With the correct Authorization header, and when the features is enabled via environmental variables, the `/aircraft` and `/callsign` routes can accept
a POST request which will update their respective entries. 

### Fixes
+ Increase api Docker memory limit, [e36915ecf467db1d972d7f98a0875e595ae615c9]

see <a href='https://github.com/mrjackwills/adsbdb/blob/main/CHANGELOG.md'>CHANGELOG.md</a> for more details
