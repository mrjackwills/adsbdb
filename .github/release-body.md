### 2026-10-12

### Chores
+ .dev.docker-compose.yml updated, [af43c8d71a4465783b85f25acc3c79d814cd7fcd]
+ .gitignore updated, [1241745ec829e2637b7898bd74538c03e43c8a98]
+ create_release updated, [a8c2d5bbf6fe8fb8220a6a9bc45c3c4918a11237]
+ dependencies updated, [182afe12cb1fff3f372b35bafc3be9fe14025037], [5504e88ea853c026a7a4136f51476bbc36c87394]
+ run.sh updated, [ac891dc9ce480f6a6e47f87e137b9c999a302da4]
+ Rust version update, [0073b69da956aad5b02f838ad72b2a34b6d1082c]
+ GitHub workflow updated, [9c8d31288f9a665af128bd726ca54c6ae007077b]

### Docs
+ comments updated, [abc57dd7db3364aa50555ef2c8b127b7998d3136], [2b5f02e2f3b00e9d65de0501ce5d1a161cace923], [349cecbade736efaa7542ee631d2cc9d73951f93], [977233e29dc5777988774dbda37702e82eebe87b]

### Features
+ improve postgres connection, [5676e457d66eebd1272a2b9d85dabaf411351887]
+ improve redis connection, [35b47317c12308d5540374c9d207b19aa1e46b92]
+ put db connection size into env, [65ab7b379f150f92a715d73a52f4e492d2b5b865]
+ use crossfire for messaging, [bcef1649d151b0e69a2d2b69ca00526327c6751a]

### Fixes
+ deserialize_image_name for new image layout, [d9a64037af7134d4d9cd318f58e02050b99c30dd]
+ improve backup creation, [9ebac1f74e09b3edf65f49780eb6a8734f93cf10]
+ insert_cache use redis transaction, [1d3269fb27c14707cddb996895a4c75acfb8a337]
+ parse_env parse err, [0d6c2caac945727e12047ba9a06a3b7564c31627]
+ photo & photo_url sql concat, [29acf572db75d12405c008cffa8b4fd69d1454aa]
+ ratelimit atomic increase, [854a541ec08f716b38a0a9213e9183ed3043a123]
+ url_photo URL bug, closes #92, [1bd993ca72bceb78b9326ffb6d7bfc0b8e9ca29d]
+ redis Dockerfile tweak, [13204480ae87d3373d677b744ca65408710d8c67], [d970915bd69bca4021b96b2256f175c5a18165b7], [fe6e006b703775c5841c11ab30cbd5c3ef5b6672]
+ redis_hash_to_struct! typo, [d58bf7817a210d25cb58f8e20113efd18c0a2659]
+ run.sh container setup, [862931ac2acbc467340d70dfd197d88b63f44558], [e0bf94c4e39c2fa08e10a9287f74d060efc9a6e4]
+ save none path temp request with "other" key, [dadbde5179479d86f97a7e3359884de49c0247ea]
+ stats init/get, [4a4a649069a1dca0808a8fe52ffeccd6eb56af77]
+ temp requests, [982a74a1c2a3d3824b22d0f17812e8aa7d7dee26]
+ use weekly rolling logs, [522b1af9128c2eba564a14f4f6eff5cd6c7f6132]

### Refactors
+ container creation & maintanence, [c510ef20cb14f48ca13bd3b6794c2bbf0dda5289]
+ de-normalize airports table, [68e06c76f306d4028e6cae589c7cd2fcd7942364]
+ incoming_request redis + added tests, [698394a4a33f35de6903229f9ac020f9b541bce1]
+ migrations.sql, [997707a221b969a15f5b585d47733c4e550868c1]
+ start_scraper, [3913da34dc79c99184eec7a5faf495fc588d8932]
+ tracing_subscriber import, [34ea2f7f8a0241e33266b56fcf79a16ed90eb1d8]

### Reverts
+ elevation null, [a0aef09ebe60c3061419503daee9ac98d1cf7c08]

### Tests
+ big test refactor, [17f3511b46a317c88f70789f723bda6420dbf9c5]
+ add stats total tests, [05a0a6ac5c2f195cfb9faff4dbb7ba3503847ef5]
+ fix broken tests, [2b1526ec7fcd0368956bb45f7af5b03c8a28e151]

see <a href='https://github.com/mrjackwills/adsbdb/blob/main/CHANGELOG.md'>CHANGELOG.md</a> for more details
