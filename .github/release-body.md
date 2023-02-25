### 2023-02-25

**This release has, potential, Breaking Changes**

### Chores
+ dev container updated, [1ac83bdb561145e101b6b3bc2c27c35471b25b50], [a398c8cc09ce4f37520137ae8f91087d55f36efd]
+ create_release updated, [eb8f871deba42e035595918c0c492e2ca4f0d156], [ea93d0b6c585d7fe5f0d050822631ad8cad46cb1]
+ dependencies updated, [01204465e1a36bbb15cf4d37cdf44398e394449c], [87c9c0e63e2e86027a07b44e031b0e1614950cdb], [a8d138e0f2775e96fa4dc6516fa905e3b007446a], [5322f1de46881984003a83d7d2063ea0172cb3da], [6e83e199ef4e99773b9d4790c11ff4098fb3abb9], [a4821b9ac28c2e563916e40c18aac8900bfc35c9]

### Docs
+ site updated, [3c4bcb49e6f0d23cc7377fcecf399f74d8067b66]
+ various comment typos, [1af07db8faaeeda55a45b386cebd851193ace79e]
+ site uptime automatically increase, [678bb062895215f6b8de3dcf6bd5e585a3a8db3a]

### Features
**Breaking Changes**
+ Callsigns & Flightroutes are now stored much more efficiently in the database, split, if possible, by IATA/ICAO prefix, and a suffix. This means that when searching for a Flightroute, one can use either the IATA or ICAO callsign.
The callsign response now includes a `callsign_iata` and `callsign_icao` key, as well as an Airline object (see the [README.md](https://github.com/mrjackwills/adsbdb/blob/main/README.md) or [adsbdb.com](https://www.adsbdb.com) for more information). A new `/airline` route is now available, which will search
for Airlines, again based on either `ICAO` or `IATA` airline codes, and will either return an array of Airlines, or a 404 response, [d1f614d3b5288dc000aa026a825e6f9f14b06f54]
+ Add an env to disable flightroute & photo scraping, [1024d7f7715f97c86a5e0ca40a906633b8f6029a]
+ Dockerfiles updated, build from source, [7c9e4861f77191d9cca904dd3c32e8ada8bae294], [2bd3df6d93505cb9132a72b0524946040f56317d]
+ openssl dependency removed, [7870c7d19c260906b1f21610a4a09dc9a5a46cad]
+ force exit if database connection error, [d950b39f0527d0419ff1219c7033ae6782d2cba3]
+ dev postgres run from /dev/shm, auto pg_dump restoration, [c5eb2466b67fa45608c8c6356389ab5f91b4aaaf], [ad171abdb487d1db90635eea866fa11ca0edaeb6]
+ backup use age, [00c9d63da8b891fdfb0b6651aef643a1b62ff4b8]

### Fixes
+ increase redis docker memory limit, [a58b6a7eaf219d2ac5c2d0becbd149b4aa1522af], [ce22824918bd56b48d077506d0edffa8dfde5905]

### Refactors
+ Rust 1.67.0 clippy linting, [b3ff5c4965f05ba0eecdb71569dc6908296d16f6]
+ dead code removed, [427bb899439b313ba3df0278f4dbc99f9d324c81]


see <a href='https://github.com/mrjackwills/adsbdb/blob/main/CHANGELOG.md'>CHANGELOG.md</a> for more details
