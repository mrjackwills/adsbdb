### 2024-04-10

### Chores
+ docker-compose version removed, [cb3f8ece38daa8fff6b8f1fb7004f104f2cd920a]
+ dependencies updated, [40421977e9804241806721e53cabe96b777d5a09]

### Features
+ use mimalloc, [f3eef71e4a9d960f6328ca9a840a3aec9abbcc1f]
+ website updated, [a84bc0221ffafa1993de41c4b87e3f1adbe349bf]

### Fixes
+ site html typo, [ed01451803bce87014ba41540853884570154660]
+ *Breaking Change* `registered_owner_operator_flag_code` is now an `Option<String>`, this was necessary due to the new data insertions as detailed below, [55da40f831ddeeab1fa1ebb9c1d9afd918fffd00]

**New Aircraft**

The number of aircraft that adsbdb contains has been expanded from 191872 to 414002, an increase of 222130. This can be broken down by country of origin to;

17 Switzerland, 25 Netherlands, 31 Singapore, 55 Isle of Man, 227 Ireland, 265 France, 840 China, 12335 Australia, 14388 United Kingdom, 18011 Canada, and 173358 United States of America. There is also an insertion of 2578 from miscellaneous sources.

The vast majority of this data comes from the publish Civil Aircraft Registries of the respective countries. However not all data from these registries is included, either due to missing data fields, or in the case of mainly the American and Canadian registries, typos – far too many typos to try to correct. 

**Flight route & other data**

The flight routes coverage has also been improved, from 171907 to 250892, an increase of 78985. To supplement this, 381 airlines and 25 airports have also been inserted.

If you are aware of any more aircraft registries available for download, or find any errors that will have undoubtedly snuck in, please feel free to report them to the [GitHub Issues](https://github.com/mrjackwills/adsbdb/issues) page

see <a href='https://github.com/mrjackwills/adsbdb/blob/main/CHANGELOG.md'>CHANGELOG.md</a> for more details
