# Metamorph

<p align="center">
  <img src="https://www.pokepedia.fr/images/e/e3/M%C3%A9tamorph-RFVF.png" width="35%" />
</p>

Just a tool to bench multiple endpoints and see the deltas between each other in HTTP and or gRPC.

## Run it

```bash
metamorph --config bench_example.toml
```

### Options

| Flag | Short | Default | Description |
|------|-------|---------|-------------|
| `--config` | `-c` | — | Path to the TOML config file (required) |
| `--read-timeout` | `-r` | `15` | HTTP read timeout in seconds |
| `--stream-max-payload` | `-s` | `1024` | Maximum payload size for stream endpoints |
| `--relative-diff` | `-d` | `0` | Maximum relative difference threshold in percentage terms for value comparisons |

## Configuration

### Unary

```toml
# Base URLs for the origin and benchmark endpoints
[origin_base]
url = "https://api.open-meteo.com"

[bench_base]
url = "https://api.open-meteo.com"

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
[origin_base]
url = "https://<example>"

[bench_base]
url = "https://<example>"

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

## gRPC

Both gRPC unary & streaming request are supported. An example of a gRPC request can be found in the [unary example configuration](./bench_example_http_grpc.toml) and [streaming example configuration](./bench_example_http_grpc_stream.toml).

> [!WARNING]
> In the case where your gRPC server is not made with tonic. It's recommended to use a protoset file to define the service. Below is an example of how to setup the protoset file.

```toml
[bench_base]
url = "http://127.0.0.1:10000"
[bench_base.method]
type = "grpc"
protoset_path = "path/to/protoset/file"
```

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
+---------------+--------+--------+--------------------------------------------+----------------+
| endpoint name | from   | target | diff                                       | deltas (in ms) |
+===============================================================================================+
| rates         | 200 OK | 200 OK | Diff detected on key: 2026-07-05T21:02:40Z | 0              |
|               |        |        | - origin: 62629.08846153847                |                |
|               |        |        | - target: 1772.8626666666669               |                |
|               |        |        | with a relative diff of 97.16926637411399% |                |
|               |        |        |                                            |                |
|               |        |        | Diff detected on key: 2026-07-05T21:02:35Z |                |
|               |        |        | - origin: 62627.402857142864               |                |
|               |        |        | - target: 1772.778                         |                |
|               |        |        | with a relative diff of 97.16932537655471% |                |
|               |        |        |                                            |                |
+---------------+--------+--------+--------------------------------------------+----------------+
```
