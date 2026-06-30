# Metamorph

<p align="center">
  <img src="https://www.pokepedia.fr/images/e/e3/M%C3%A9tamorph-RFVF.png" width="35%" />
</p>

Just a tool to bench multiple endpoints and see the deltas between each other.

## Run it

```bash
cargo run --release -- --config bench_example.toml
```

### Options

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--config` | `-c` | — | Path to the TOML config file (required) |
| `--read-timeout` | `-r` | `15` | HTTP read timeout in seconds |
| `--stream-max-payload` | `-s` | `1024` | Maximum payload size for stream endpoints |

## Configuration

### Unary

```toml
# Base URLs for the origin and benchmark endpoints
origin_base_url = "https://api.open-meteo.com"
bench_base_url = "https://api.open-meteo.com"

[forecast]

[forecast.from]
path = "v1/forecast?latitude={lat}&longitude={lon}&hourly={hourly}&start_date={start}&end_date={end}&temperature_unit={unit}"

[forecast.from.params]
lat = "48.85"
lon = "2.35"
hourly = "temperature_2m"
start = "2026-06-19"
end = "2026-06-20"
unit = "celsius"

[forecast.target]
path = "v1/forecast?latitude={lat}&longitude={lon}&hourly={hourly}&start_date={start}&end_date={end}&temperature_unit={unit}"

[forecast.target.params]
lat = "48.85"
lon = "2.35"
hourly = "temperature_2m"
start = "2026-06-19"
end = "2026-06-20"
unit = "fahrenheit"

# Then could be more below...
```

### Stream

```toml
origin_base_url = "https://<example>"
bench_base_url = "https://<example>"
stream = true

[headers.origin.api_key]
name = "key"
value = "xxxx"

[headers.bench.api_key]
name = "key"
value = "xxxx"

[rates]

[rates.from]
path = "api/stream/index_v1"
method = "Post"
check_path = "$.result.percentages[0].price"
reconcile_path = "$.result.interval.endTime"

[rates.from.params]
args = '{"indexCode": "KK_BRR_BTCUSD"}'

[rates.target]
path = "api/stream/index_v1"
method = "Post"
check_path = "$.result.percentages[0].price"
reconcile_path = "$.result.interval.endTime"

[rates.target.params]
args = '{"indexCode": "KK_BRR_ETHUSD"}'
```

## Output example

### Success no diff

```sh
✔ Finished processing forecast endpoints.
+---------------+--------+--------+------+----------------+
| endpoint name | from   | target | diff | deltas (in ms) |
+=========================================================+
| forecast      | 200 OK | 200 OK | None | 5              |
+---------------+--------+--------+------+----------------+
```

### Success with diff

```sh
✔ Finished processing rates endpoints.
+---------------+--------+--------+--------------------------------------------------------------------------------------------+----------------+
| endpoint name | from   | target | diff                                                                                       | deltas (in ms) |
+===============================================================================================================================================+
| rates         | 200 OK | 200 OK | Diff on key: 2026-06-23T22:40:00Z, origin: 1662.95875 vs target: 62496.735                 | 0              |
|               |        |        | Diff on key: 2026-06-23T22:40:05Z, origin: 1662.773 vs target: 62494.02                    |                |
|               |        |        | Diff on key: 2026-06-23T22:39:55Z, origin: 1663.0258333333331 vs target: 62497.97266666667 |                |
+---------------+--------+--------+--------------------------------------------------------------------------------------------+----------------+
```
