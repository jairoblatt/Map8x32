# Map8x32

A high-performance in-memory key-value store using Unix domain sockets and a custom binary protocol.

## Overview

Map8x32 is a lightweight, concurrent key-value database designed for maximum performance. It stores collections of 32-bit unsigned integers indexed by 8-bit keys, making it ideal for scenarios requiring fast lookups with small key spaces.

## Architecture

### Key Features
- **Unix Domain Sockets**: Eliminates TCP/IP overhead for local communication
- **Custom Binary Protocol**: Minimal 6-byte request format for zero parsing overhead
- **Concurrent HashMap**: Thread-safe operations using DashMap
- **Async I/O**: Built on Tokio for high concurrency
- **Zero-Copy Operations**: Direct binary data handling without serialization

### Protocol Specification

**Request Format** (6 bytes):
```
[operation: u8][key: u8][value: u32 little-endian]
```

**Operations**:
- `1` = SET: Store value in key's collection
- `2` = GET: Retrieve all values for key

**Response Format**:
- SET: `[status: u8]` (1=OK, 0=NOT_FOUND, 2=BAD_REQUEST)
- GET: `[status: u8][count: u32][values: u32...]`


### Benchmark
```txt
Running test with 10 concurrent users for 10s
Starting benchmark test...
Socket: /tmp/map8x32.sock
Max concurrent users: 10
Duration: 10s

Benchmark Results:
==================
Total requests: 44596
Successful requests: 0
Failed requests: 44596
Success rate: 0.00%
Requests per second: 4458.31
Duration: 10.0028876s

Response Times:
  Average: 33.689µs
  Minimum: 12.554µs
  Maximum: 885.881µs
  95th percentile: 69.382µs
==================================================
Running test with 50 concurrent users for 20s
Starting benchmark test...
Socket: /tmp/map8x32.sock
Max concurrent users: 50
Duration: 20s

Benchmark Results:
==================
Total requests: 425214
Successful requests: 0
Failed requests: 425214
Success rate: 0.00%
Requests per second: 21252.53
Duration: 20.007685439s

Response Times:
  Average: 25.317µs
  Minimum: 12.193µs
  Maximum: 1.093697ms
  95th percentile: 52.179µs
==================================================
Running test with 100 concurrent users for 15s
Starting benchmark test...
Socket: /tmp/map8x32.sock
Max concurrent users: 100
Duration: 15s

Benchmark Results:
==================
Total requests: 565624
Successful requests: 0
Failed requests: 565624
Success rate: 0.00%
Requests per second: 37676.95
Duration: 15.012466078s

Response Times:
  Average: 21.77µs
  Minimum: 12.193µs
  Maximum: 7.443448ms
  95th percentile: 36.009µs
==================================================
Running test with 50 concurrent users for 10s
Starting benchmark test...
Socket: /tmp/map8x32.sock
Max concurrent users: 50
Duration: 10s

Benchmark Results:
==================
Total requests: 214305
Successful requests: 0
Failed requests: 214305
Success rate: 0.00%
Requests per second: 21424.70
Duration: 10.002705831s

Response Times:
  Average: 23.945µs
  Minimum: 12.274µs
  Maximum: 767.618µs
  95th percentile: 47.7µs
==================================================

```

## Performance Results

### Benchmark Configuration
- **Test Environment**: Unix sockets, binary protocol
- **Concurrency**: 10-100 simultaneous connections
- **Operations**: Mixed SET/GET with data validation
- **Key Range**: 0-127 (u8)
- **Value Range**: 0-4,294,967,295 (u32)

## Usage

### Starting the Server
```bash
cd server
cargo run --release
```

### Running Benchmarks
```bash
cd benchmark
cargo run
```

### Client Integration
Connect to `/tmp/map8x32.sock` and send 6-byte binary requests:

```rust
// Example SET operation: key=42, value=1337
let request = [1, 42, 0x39, 0x05, 0x00, 0x00]; // op=1, key=42, value=1337
stream.write_all(&request).await?;
let status = stream.read_u8().await?; // 1 = success
```

## Dependencies

### Server
- `dashmap`: Concurrent hashmap implementation
- `tokio`: Async runtime

### Benchmark
- `tokio`: Async runtime  
- `fastrand`: Random number generation

## Architecture Benefits

1. **Low Latency**: Binary protocol eliminates parsing overhead
2. **High Throughput**: Unix sockets reduce network stack overhead
3. **Memory Efficient**: Direct binary operations, no string allocations
4. **Scalable**: Lock-free concurrent operations via DashMap
5. **Simple**: Minimal dependencies and straightforward protocol

## Use Cases

- High-frequency trading systems
- Real-time analytics caching
- Gaming leaderboards and statistics
- IoT sensor data aggregation
- Any scenario requiring sub-millisecond key-value operations



