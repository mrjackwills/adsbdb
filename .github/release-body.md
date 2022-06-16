### 2022-06-16

### Docs
+ Add twitter links to Readme.md & site, [904259a21018e34e5b73696758af574a2e17f768]

### Features
+ Return Result<(), AppError> in main(), [8ca3ad3e3ffb0dc8dd4962042c3989902b7dbb68]
+ Use match in ratelimit middleware, to remove unwrap(), [2ea55d12d733be51d210cdceb2a8ef82d1a7bea6]
+ Add connection timeouts to postgres & redis, [5e242efa7883329efe95b7659a4cf81a63e3a8d3]
+ create_release.sh update api.Dockerfile to download latest build from github, [9f29591d0018a499a982e380738bdff676dff456]

### Fixes
+ Remove ModelAicraft get unwrap, [2a28831c58bf1b02915e9b6b49ff330864778a67]

### Refactors
+ Route handlers renamed from method_routename to routename_method, [aaba380cb217ba5ee3ca0142f1c4131eeb0a2692]
+ unused code removed, [879afd0c2ff6ce4bf4b42c2b0c2a793221ddfd02]

### Tests
+ Use a TestSetup struct to contain test data, impl finish() to close db connection at end of test, [693bb60c66fdf7d53e839d4f875d552d44b6ff76]



see <a href='https://github.com/mrjackwills/adsbdb/blob/main/CHANGELOG.md'>CHANGELOG.md</a> for more details
