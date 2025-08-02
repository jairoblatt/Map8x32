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




## Performance Results

```txt
MAP8X32 BENCHMARK
=================
SET Operations:
  50000 ops - min: 76μs, avg: 114μs, max: 849μs, p99: 194μs (8748 ops/sec)
GET Operations:
  50000 ops - min: 523μs, avg: 739μs, max: 5461μs, p99: 1202μs (1352 ops/sec)
DELETE Operations:
  50000 ops - min: 75μs, avg: 105μs, max: 1170μs, p99: 178μs (9493 ops/sec)
LIST Operations:
  50 ops - min: 134μs, avg: 147μs, max: 229μs, p99: 229μs (6774 ops/sec)
Consistency Test:
  PASS
Concurrent Test (20 workers, 100 ops each):
  2000 ops - min: 243μs, avg: 885μs, max: 2122μs, p99: 1566μs (1130 ops/sec)
```

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



