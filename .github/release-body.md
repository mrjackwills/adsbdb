### 2022-07-21

### Chores
+  dependancies updated, [81334ac97569d011613e81a366aa42eb28efc0fc]

### Features
+ parse server bind_address from env.host_name & env.port, [1f66fb84a4825d2ba9e17a9e16fdded0f33ebdc0]
+ parse env from file, closes [#4], [e3d0429fe359cfbf5050c090835b97e5cb0ce588]
+ log to file and stdout, [c709446db67d31ce4235cb226ff920a532caa329]
+ api.dev/Dockerfile added, [97f3fe03db9f1c5c0b579cc68b1b63035f987e64]
+ redis conf create using .env, [a4c30f3ba96cb3d5efcdc57abf7af743b723f743]

### Fixes
+ Rate limit only set ttl if limit has been hit, or no key exists, [acb51fee4de826f11cfa59d37f385a426c4b5ccd]
+ change docker mount locations of databases & logs, [710066a250dd364c8418395b121fa5d7767ce0b8]

### Refactors
+ run.sh & create_release.sh updated, [8855e03e384882606c00a2e4b3f028e13f1d2f83]


see <a href='https://github.com/mrjackwills/adsbdb/blob/main/CHANGELOG.md'>CHANGELOG.md</a> for more details
