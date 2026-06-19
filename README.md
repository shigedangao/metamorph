# Metamorph

Just a tool to bench 2 endpoints and see the deltas between them.

## Run it

```bash
cargo run --release  -- --config bench_example.toml
```

## Configuration

```toml
# Base URLs for the origin and benchmark endpoints
origin_base_url = "https://api.open-meteo.com"
bench_base_url = "https://api.open-meteo.com"
headers = {}

# Endpoint configuration
[forecast]

[forecast.from]
endpoint = "v1/forecast?latitude={lat}&longitude={lon}&hourly={hourly}&start_date={start}&end_date={end}&temperature_unit={unit}"
params = {
    lat = "48.85",
    lon = "2.35",
    hourly = "temperature_2m",
    start = "2026-06-19",
    end = "2026-06-20",
    unit = "celsius",
}

[forecast.target]
endpoint = "v1/forecast?latitude={lat}&longitude={lon}&hourly={hourly}&start_date={start}&end_date={end}&temperature_unit={unit}"
params = {
    lat = "48.85",
    lon = "2.35",
    hourly = "temperature_2m",
    start = "2026-06-19",
    end = "2026-06-20",
    unit = "fahrenheit",
}

# Then could be more below...
```

## Output example

```sh
✔ Finished processing forecast endpoints.
+---------------+--------+--------+----------------+
| endpoint name | from   | target | deltas (in ms) |
+==================================================+
| forecast      | 200 OK | 200 OK | 0              |
+---------------+--------+--------+----------------+
```
