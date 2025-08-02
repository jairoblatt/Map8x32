use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::time::timeout;

const SOCKET_PATH: &str = "/tmp/map8x32.sock";
const OP_SET: u8 = 1;
const OP_GET: u8 = 2;
const OP_DELETE_BY_KEY: u8 = 3;
const OP_DELETE_ALL: u8 = 4;
const OP_LIST_ALL: u8 = 5;

async fn send_op(op: u8, key: u8, value: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut stream = UnixStream::connect(SOCKET_PATH).await?;
    let mut buf = [0u8; 6];
    buf[0] = op;
    buf[1] = key;
    buf[2..6].copy_from_slice(&value.to_le_bytes());
    stream.write_all(&buf).await?;

    let status = match timeout(Duration::from_secs(1), stream.read_u8()).await {
        Ok(Ok(s)) => s,
        _ => return Err("Timeout or error reading status".into()),
    };
    let mut response = vec![status];

    match op {
        OP_GET if status == 1 => {
            let count = stream.read_u32_le().await?;
            response.extend_from_slice(&count.to_le_bytes());
            for _ in 0..count {
                let value = stream.read_u32_le().await?;
                response.extend_from_slice(&value.to_le_bytes());
            }
        }
        OP_LIST_ALL if status == 1 => {
            let key_count = stream.read_u32_le().await?;
            response.extend_from_slice(&key_count.to_le_bytes());
            for _ in 0..key_count {
                let key = stream.read_u8().await?;
                response.push(key);
                let value_count = stream.read_u32_le().await?;
                response.extend_from_slice(&value_count.to_le_bytes());
                for _ in 0..value_count {
                    let value = stream.read_u32_le().await?;
                    response.extend_from_slice(&value.to_le_bytes());
                }
            }
        }
        _ => {}
    }

    Ok(response)
}

async fn set_test(iterations: u32) -> (u32, Vec<u64>) {
    let mut successes = 0;
    let mut times = Vec::new();

    for i in 0..iterations {
        let op_start = Instant::now();
        if let Ok(resp) = send_op(OP_SET, (i % 256) as u8, i).await {
            times.push(op_start.elapsed().as_micros() as u64);
            if !resp.is_empty() && resp[0] == 1 {
                successes += 1;
            }
        }
    }

    times.sort();
    (successes, times)
}

async fn get_test(iterations: u32) -> (u32, Vec<u64>) {
    let mut successes = 0;
    let mut times = Vec::new();

    for i in 0..iterations {
        let op_start = Instant::now();
        if let Ok(resp) = send_op(OP_GET, (i % 256) as u8, 0).await {
            times.push(op_start.elapsed().as_micros() as u64);
            if !resp.is_empty() {
                successes += 1;
            }
        }
    }

    times.sort();
    (successes, times)
}

async fn delete_test(iterations: u32) -> (u32, Vec<u64>) {
    let mut successes = 0;
    let mut times = Vec::new();

    for i in 0..iterations {
        let op_start = Instant::now();
        if let Ok(resp) = send_op(OP_DELETE_BY_KEY, (i % 256) as u8, 0).await {
            times.push(op_start.elapsed().as_micros() as u64);
            if !resp.is_empty() {
                successes += 1;
            }
        }
    }

    times.sort();
    (successes, times)
}

async fn list_test(iterations: u32) -> (u32, Vec<u64>) {
    let mut successes = 0;
    let mut times = Vec::new();

    for _ in 0..iterations {
        let op_start = Instant::now();
        if let Ok(resp) = send_op(OP_LIST_ALL, 0, 0).await {
            times.push(op_start.elapsed().as_micros() as u64);
            if !resp.is_empty() && resp[0] == 1 {
                successes += 1;
            }
        }
    }

    times.sort();
    (successes, times)
}

async fn consistency_test() -> bool {
    let key = 42u8;
    let value = 12345u32;

    if send_op(OP_SET, key, value).await.is_err() {
        return false;
    }

    let get_resp = send_op(OP_GET, key, 0).await;
    if get_resp.is_err() {
        return false;
    }
    let resp = get_resp.unwrap();
    if resp.len() < 5 || resp[0] != 1 {
        return false;
    }

    let count = u32::from_le_bytes([resp[1], resp[2], resp[3], resp[4]]);
    if count == 0 || resp.len() < 5 + (count as usize * 4) {
        return false;
    }

    let stored_value = u32::from_le_bytes([resp[5], resp[6], resp[7], resp[8]]);
    if stored_value != value {
        return false;
    }

    if send_op(OP_DELETE_BY_KEY, key, 0).await.is_err() {
        return false;
    }

    let get_resp2 = send_op(OP_GET, key, 0).await;
    if get_resp2.is_err() {
        return false;
    }
    let resp2 = get_resp2.unwrap();
    resp2.is_empty() || resp2[0] == 0
}

async fn concurrent_test(workers: u32, ops_per_worker: u32) -> (u32, Vec<u64>) {
    let mut handles = Vec::new();
    let mut all_times = Vec::new();

    for worker_id in 0..workers {
        let handle = tokio::spawn(async move {
            let mut local_successes = 0;
            let mut local_times = Vec::new();
            for i in 0..ops_per_worker {
                let key = ((worker_id * ops_per_worker + i) % 256) as u8;
                let value = worker_id * 1000 + i;

                let op_start = Instant::now();
                if send_op(OP_SET, key, value).await.is_ok() {
                    if let Ok(resp) = send_op(OP_GET, key, 0).await {
                        local_times.push(op_start.elapsed().as_micros() as u64);
                        if !resp.is_empty() && resp[0] == 1 {
                            local_successes += 1;
                        }
                    }
                }
            }
            (local_successes, local_times)
        });
        handles.push(handle);
    }

    let mut total_successes = 0;
    for handle in handles {
        if let Ok((successes, mut times)) = handle.await {
            total_successes += successes;
            all_times.append(&mut times);
        }
    }

    all_times.sort();
    (total_successes, all_times)
}

fn print_stats(name: &str, success: u32, times: &[u64]) {
    if times.is_empty() {
        println!("{}: No operations completed", name);
        return;
    }

    let min = times[0];
    let max = times[times.len() - 1];
    let avg = times.iter().sum::<u64>() / times.len() as u64;
    let p99_idx = ((times.len() as f64 * 0.99) as usize).min(times.len() - 1);
    let p99 = times[p99_idx];
    let total_time = times.iter().sum::<u64>();
    let ops_per_sec = if total_time > 0 {
        (success as f64 * 1_000_000.0) / total_time as f64
    } else {
        0.0
    };

    println!("{}:", name);
    println!(
        "  {} ops - min: {}μs, avg: {}μs, max: {}μs, p99: {}μs ({:.0} ops/sec)",
        success, min, avg, max, p99, ops_per_sec
    );
}

#[tokio::main]
async fn main() {
    println!("MAP8X32 BENCHMARK");
    println!("=================");

    let iterations = 50_000;

    let (set_success, set_times) = set_test(iterations).await;
    print_stats("SET Operations", set_success, &set_times);

    let (get_success, get_times) = get_test(iterations).await;
    print_stats("GET Operations", get_success, &get_times);

    let (del_success, del_times) = delete_test(iterations).await;
    print_stats("DELETE Operations", del_success, &del_times);

    let (list_success, list_times) = list_test(50).await;
    print_stats("LIST Operations", list_success, &list_times);

    println!("Consistency Test:");
    let consistent = consistency_test().await;
    println!("  {}", if consistent { "PASS" } else { "FAIL" });

    let (conc_success, conc_times) = concurrent_test(20, 100).await;
    print_stats(
        "Concurrent Test (20 workers, 100 ops each)",
        conc_success,
        &conc_times,
    );
}
