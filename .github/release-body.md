### 2025-10-13

### Chores
+ create_release.sh updated, [17bbef124551caa50db703038263901796aad0ea], [c2a24e8392493a099edd245c0e893aa816600911]
+ dependencies updated, [3a709dffc3ade45f1e5b73756d74fd8c63535952], [598bb3a3d4acca4ae2ff546659799e08f8a26dab], [8faa4c0f17127c430c6ded8bf2c45319b80558a5], [9ae4031e3f7c64a01ba0e8de9815c3ba7d236cd3]
+ Dockerfile version bump, [3ae22eca2e9c7e5cc0868b67091c404cc32163d9]
+ GitHub workflow dependencies updated, [200ac036563e84226523db3907586350808946ab], [c7069cb31d07b8cd414139f6bbb43d2dede049a8]
+ run.sh updated, [65d3b444cde632ed99d0c894d7a7eb2b3607ec78]
+ Rust 1.90.0 linting, [7da5c6d257d671a98a47f61342f2ea98cce74813]

### Docs
+ comments updated/added, [44a093d7da5e72ea77e94658668bbeba670e739c]
+ readme updated, [b381ff6ef277d41ca57d06a6273a2641e03536ee]

### Features
+ `/stats` route, lists some basic usage statistics of the `api.adbsbd.com` service. Only the URL of a request is stored, no other details are recorded. Handling an average of 1 million requests a day, all with ease, [72b11ef5bdaca48a661413983ae153ff7029cd13], [29eb19b46846b5a5c8ab35e30988c29754dda1d1]
+ `/aircraft/random`, `/airline/random`, `/callsign/random`, endpoints added. Get a randomised value at these addresses, [8f78f7c50b333969e543f419c5cfeafeae35f035]
+ see the new [adsbdb.com](https://www.adsbdb.com) website for a more detailed descriptions of all the endpoints.

### Refactors
+ `site` dir removed, new [adsbdb_site](https://www.github.com/mrjackwills/adsbdb_site), now live at [adsbdb.com](https://www.adsbdb.com), [e95b9f174f06a99454d4f676f296315b0058d0b5]

see <a href='https://github.com/mrjackwills/adsbdb/blob/main/CHANGELOG.md'>CHANGELOG.md</a> for more details
