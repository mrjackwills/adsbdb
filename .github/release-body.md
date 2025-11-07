### 2025-11-07

### Chores
+ dependecies updated, [f6578306d160ce7c47b59e55f0b92002b5ffe64c]
+ run.sh & init_postgres.sh updated, [5e4633f14fa0d79a49816e2cb6bcd2780cba9288]
+ update api.Dockerfile, [22ed0345a22c85edda40320f6df88657740339b2]
+ update redis port, [552bacc67677de54a8e4eb48b2d44dd60bb0a2b0]

### Docs
+ test comment, [038ebe8c3b4b9f0859e51ac1fd2bfe34d68e2137]

### Features
+ Independent database connections for each thread, [21bcdf1cff29faecbda68d9ee927f34fb8cef9ee]
+ update random flightroute, [e7f55bf6f6cfd9f21c0d9289d7ffad3248d27ca9]
+ use bounded async_channel instead of tokio::sync::mpsc, [bcc32f098dec8a13ffb2379613033115ff8b4d71]

### Fixes
+ allow '+' in registrations, closes #60, [14a4653d7707243bdb9cc11780e69faeb652d36b]
+ remove /ramdata usage, [2c12417e9a7ac936863cf5a3388b096d2c4f0e6d]
+ set max_wal_size using docker-compose.yml, [2972cc5254767fb0957979a0ddc6bd48e96c0f9b]
+ transparent errors, [de44d6ef6dde3388a9dd4f49fb2d286f08edea03]

### Refactors
+ incoming_request_url table, [59f9c28a093418410db5856f9681278841f62e58]
+ Incoming request remove transactions, [1825222454a9fe29e370d2e9705704f113e16819]
+ use try_joins, [273ba3ba0bb9cece30a1809901f5fdf89a78c42f]

### Tests
+ additional registration validate tests, [e1a609dbc5fa0bb10bca7f5b1ac97ad5370c003b]

see <a href='https://github.com/mrjackwills/adsbdb/blob/main/CHANGELOG.md'>CHANGELOG.md</a> for more details
