<p align="center">
 <img src='./.github/logo.svg' width='200px'/>
</p>

<p align="center">
 <h1 align="center">api.adsbdb.com</h1>
</p>

<p align="center">
	aircraft & flightroute api
</p>

<p align="center">
	Built in <a href='https://www.rust-lang.org/' target='_blank' rel='noopener noreferrer'>Rust</a>
	for <a href='https://www.docker.com/' target='_blank' rel='noopener noreferrer'>Docker</a>,
	using <a href='https://www.postgresql.org/' target='_blank' rel='noopener noreferrer'>PostgreSQL</a>
	& <a href='https://www.redis.io/' target='_blank' rel='noopener noreferrer'>Redis</a> 
</p>


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

```https://api.adsbdb.com/v[semver.major]/callsign/[CALLSIGN]```
```json
{
	"response": {
		"flightroute":{
			"callsign": string,
			"origin_airport_country_iso_name": string,
			"origin_airport_country_name": string,
			"origin_airport_elevation": number,
			"origin_airport_iata_code": string,
			"origin_airport_icao_code": string,
			"origin_airport_latitude": number,
			"origin_airport_longitude": number,
			"origin_airport_municipality": string,
			"origin_airport_name": string,

			"destination_airport_country_iso_name": string,
			"destination_airport_country_name": string,
			"destination_airport_elevation": number,
			"destination_airport_iata_code": string,
			"destination_airport_icao_code": string,
			"destination_airport_latitude": number,
			"destination_airport_longitude": number,
			"destination_airport_municipality": string,
			"destination_airport_name": string
		}
	}
}
```

For a small number of flightroutes, midpoints are also included
```json
{
	"midpoint_airport_country_iso_name": string,
	"midpoint_airport_country_name": string,
	"midpoint_airport_elevation": number,
	"midpoint_airport_iata_code": string,
	"midpoint_airport_icao_code": string,
	"midpoint_airport_latitude": number,
	"midpoint_airport_longitude": number,
	"midpoint_airport_municipality": string,
	"midpoint_airport_name": string
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
			"registered_owner_country_iso_name": string,
			"registered_owner_country_name": string,
			"registered_owner_operator_flag_code": string,
			"registered_owner": string,
			"url_photo":string || null,
			"url_photo_thumbnail":string || null
		},
		"flightroute":{
			"callsign": string,
			"origin_airport_country_iso_name": string,
			"origin_airport_country_name": string,
			"origin_airport_elevation": number,
			"origin_airport_iata_code": string,
			"origin_airport_icao_code": string,
			"origin_airport_latitude": number,
			"origin_airport_longitude": number,
			"origin_airport_municipality": string,
			"origin_airport_name": string,
			"destination_airport_country_iso_name": string,
			"destination_airport_country_name": string,
			"destination_airport_elevation": number,
			"destination_airport_iata_code": string,
			"destination_airport_icao_code": string,
			"destination_airport_latitude": number,
			"destination_airport_longitude": number,
			"destination_airport_municipality": string,
			"destination_airport_name": string
		}
	}
}
```

If an unknown callsign is provided as a query param, but the aircraft is known, response will be status 200 with just aircraft

---

### Run

Operate docker compose containers via

```bash
./run.sh
```

## Tests

Requires postgres & redis to both be operational and seeded with data


```bash
# Watch
cargo watch -q -c -w src/ -x 'test  -- --test-threads=1 --nocapture'

# Run all 
cargo test -- --test-threads=1 --nocapture
```