<p align="center">
	<img src='./.github/logo.svg' width='125px'/>
	<h1 align="center"><a href='https://api.adsbdb.com' target='_blank' rel='noopener noreferrer'>api.adsbdb.com</a></h1>
</p>

<p align="center">
	public aircraft & flightroute api<br>
	Built in <a href='https://www.rust-lang.org/' target='_blank' rel='noopener noreferrer'>Rust</a>,
	with <a href='https://github.com/tokio-rs/axum' target='_blank' rel='noopener noreferrer'>axum</a>,
	for <a href='https://www.docker.com/' target='_blank' rel='noopener noreferrer'>Docker</a>,
	using <a href='https://www.postgresql.org/' target='_blank' rel='noopener noreferrer'>PostgreSQL</a>
	& <a href='https://www.redis.io/' target='_blank' rel='noopener noreferrer'>Redis</a> 
	<br>
	<sub> See typescript branch for original typescript version</sub>
</p>

<hr>

<p>
	check <a href='https://twitter.com/adsbdb' target='_blank' rel='noopener noreferrer'>adsbdb twitter</a> for any status updates,
	and please report any incorrect data to the <a href="https://github.com/mrjackwills/adsbdb/issues/new/choose" target='_blank' rel='noopener noreferrer'>issues page</a>, with the <strong>Data</strong> tag.
	<br>
	With thanks to;
	<li>
		<a href="http://planebase.biz/" target='_blank' rel='noopener noreferrer'>PlaneBase</a> for the aircraft data.
	</li>
	<li>
		The flight route data is the work of David Taylor, Edinburgh and Jim Mason, Glasgow, and may not be copied, published, or incorporated into other databases without the explicit permission of David J Taylor, Edinburgh.
	</li>
	<li>
		<a href="https://github.com/guillaumemichel/icao-nnumber_converter" target='_blank' rel='noopener noreferrer'>Guillaume Michel</a>, for the icao to n-number conversion 
	</li>
	<li>
		<a href='https://www.airport-data.com' target='_blank' rel='noopener noreferrer'>airport-data</a> for aircraft photographs
	</li>
</p>
<hr>


## Routes

```https://api.adsbdb.com/v[semver.major]/aircraft/[MODE_S]```
```json
{
	"response":{
		"aircraft":{
			"type": string,
			"icao_type": string,
			"manufacturer": string,
			"mode_s": string,
			"n_number": string,
			"registered_owner_country_iso_name": string,
			"registered_owner_country_name": string,
			"registered_owner_operator_flag_code": string,
			"registered_owner": string,
			"url_photo":string || null,
			"url_photo_thumbnail":string || null
		}
	}
}

```

Unknown aircraft return status 404 with
```json
{ "response": "unknown aircraft"}
```
---

Convert from MODE-S string to N-Number string
```https://api.adsbdb.com/v[semver.major]/mode-s/[MODE_S]```
```json
{
	"response": string
}

```
---

Convert from N-Number string to Mode_S string
```https://api.adsbdb.com/v[semver.major]/n-number/[N-NUMBER]```
```json
{
	"response": string
}

```
---

```https://api.adsbdb.com/v[semver.major]/callsign/[CALLSIGN]```
```json
{
	"response": {
		"flightroute":{
			"callsign": string,

			"origin": {
				"country_iso_name": string,
				"country_name": string,
				"elevation": number,
				"iata_code": string,
				"icao_code": string,
				"latitude": number,
				"longitude": number,
				"municipality": string,
				"name": string,
			},

			"destination": {
				"country_iso_name": string,
				"country_name": string,
				"elevation": number,
				"iata_code": string,
				"icao_code": string,
				"latitude": number,
				"longitude": number,
				"municipality": string,
				"name": string,
			}
		}
	}
}
```

For a small number of flightroutes, midpoints are also included
```json
	{
		"midpoint": {
				"country_iso_name": string,
				"country_name": string,
				"elevation": number,
				"iata_code": string,
				"icao_code": string,
				"latitude": number,
				"longitude": number,
				"municipality": string,
				"name": string,
			}
	}
```

Unknown callsign return status 404 with
```json
{ "response": "unknown callsign"}
```
---

```https://api.adsbdb.com/v[semver.major]/aircraft/[MODE_S]?callsign=[CALLSIGN]``` 

```json
{
	"response": {
		
		"aircraft":{
			"type": string,
			"icao_type": string,
			"manufacturer": string,
			"mode_s": string,
			"n_number": string,
			"registered_owner_country_iso_name": string,
			"registered_owner_country_name": string,
			"registered_owner_operator_flag_code": string,
			"registered_owner": string,
			"url_photo":string || null,
			"url_photo_thumbnail":string || null
		},

		"flightroute":{
			"callsign": string,

			"origin": {
				"country_iso_name": string,
				"country_name": string,
				"elevation": number,
				"iata_code": string,
				"icao_code": string,
				"latitude": number,
				"longitude": number,
				"municipality": string,
				"name": string,
			},

			"destination": {
				"country_iso_name": string,
				"country_name": string,
				"elevation": number,
				"iata_code": string,
				"icao_code": string,
				"latitude": number,
				"longitude": number,
				"municipality": string,
				"name": string,
			}
		}
	}
}
```

If an unknown callsign is provided as a query param, but the aircraft is known, response will be status 200 with just aircraft

---

## Download

See <a href="https://github.com/mrjackwills/adsbdb/releases" target='_blank' rel='noopener noreferrer'>releases</a>

download one liner

```bash
wget https://www.github.com/mrjackwills/adsbdb/releases/latest/download/adsbdb_linux_x86_64.tar.gz &&
tar xzvf adsbdb_linux_x86_64.tar.gz adsbdb
```

### Run

Operate docker compose containers via

```bash
./run.sh
```


### Build

```bash
cargo build --release
```
<strike>
Build using <a href='https://github.com/cross-rs/cross' target='_blank' rel='noopener noreferrer'>cross</a>, for x86_64 linux musl targets, in order to run in an Alpine based container

```bash
cross build --target x86_64-unknown-linux-musl --release
```
</strike>



## Tests

Requires postgres & redis to both be operational and seeded with valid data

```bash
# Watch
cargo watch -q -c -w src/ -x 'test  -- --test-threads=1 --nocapture'

# Run all 
cargo test -- --test-threads=1 --nocapture
```