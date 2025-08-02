use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::sync::Semaphore;
use tokio::time::sleep;

#[derive(Debug)]
struct BenchmarkResults {
    total_requests: u64,
    successful_requests: u64,
    failed_requests: u64,
    avg_response_time: Duration,
    min_response_time: Duration,
    max_response_time: Duration,
    p95_response_time: Duration,
    requests_per_second: f64,
    duration: Duration,
}

async fn send_set_request(
    socket_path: &str,
    key: u8,
    value: u32,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let mut stream = UnixStream::connect(socket_path).await?;

    // Send: [op=1, key, value_bytes[4]]
    let mut buf = [0u8; 6];
    buf[0] = 1; // SET operation
    buf[1] = key;
    buf[2..6].copy_from_slice(&value.to_le_bytes());

    stream.write_all(&buf).await?;

    // Read response
    let response = stream.read_u8().await?;
    Ok(response == 1) // OK
}

async fn send_get_request(
    socket_path: &str,
    key: u8,
) -> Result<Option<Vec<u32>>, Box<dyn std::error::Error + Send + Sync>> {
    let mut stream = UnixStream::connect(socket_path).await?;

    // Send: [op=2, key, dummy_value[4]]
    let mut buf = [0u8; 6];
    buf[0] = 2; // GET operation
    buf[1] = key;
    // buf[2..6] can be anything for GET

    stream.write_all(&buf).await?;

    // Read response
    let status = stream.read_u8().await?;
    if status == 1 {
        // OK
        let count = stream.read_u32_le().await?;
        let mut values = Vec::with_capacity(count as usize);
        for _ in 0..count {
            values.push(stream.read_u32_le().await?);
        }
        Ok(Some(values))
    } else {
        Ok(None) // NOT_FOUND or error
    }
}

async fn benchmark_post_get(
    socket_path: &str,
    key: u8,
    value: u32,
) -> Result<(Duration, bool), Box<dyn std::error::Error + Send + Sync>> {
    let start = Instant::now();

    let set_success = send_set_request(socket_path, key, value)
        .await
        .unwrap_or(false);

    if !set_success {
        return Ok((start.elapsed(), false));
    }

    let get_result = send_get_request(socket_path, key).await.unwrap_or(None);

    let get_success = match get_result {
        Some(values) => values.contains(&value),
        None => false,
    };

    Ok((start.elapsed(), set_success && get_success))
}

async fn run_benchmark_worker(
    socket_path: String,
    semaphore: Arc<Semaphore>,
    duration: Duration,
) -> (u64, u64, Vec<Duration>) {
    let mut successful = 0;
    let mut failed = 0;
    let mut response_times = Vec::new();
    let start_time = Instant::now();

    while start_time.elapsed() < duration {
        let _permit = semaphore.acquire().await.unwrap();

        let key = fastrand::u8(0..128);
        let value = fastrand::u32(0..u32::MAX);

        match benchmark_post_get(&socket_path, key, value).await {
            Ok((duration, success)) => {
                response_times.push(duration);
                if success {
                    successful += 1;
                } else {
                    failed += 1;
                }
            }
            Err(_) => {
                failed += 1;
            }
        }

        sleep(Duration::from_millis(1)).await;
    }

    (successful, failed, response_times)
}

async fn run_load_test(
    socket_path: &str,
    max_concurrent: usize,
    duration: Duration,
) -> BenchmarkResults {
    println!("Starting benchmark test...");
    println!("Socket: {}", socket_path);
    println!("Max concurrent users: {}", max_concurrent);
    println!("Duration: {:?}", duration);
    println!();

    let semaphore = Arc::new(Semaphore::new(max_concurrent));
    let socket_path = socket_path.to_string();

    let start_time = Instant::now();

    let mut tasks = Vec::new();
    for _ in 0..max_concurrent {
        let socket_path = socket_path.clone();
        let semaphore = semaphore.clone();

        tasks.push(tokio::spawn(run_benchmark_worker(
            socket_path,
            semaphore,
            duration,
        )));
    }

    let mut total_successful = 0;
    let mut total_failed = 0;
    let mut all_response_times = Vec::new();

    for task in tasks {
        let (successful, failed, mut response_times) = task.await.unwrap();
        total_successful += successful;
        total_failed += failed;
        all_response_times.append(&mut response_times);
    }

    let actual_duration = start_time.elapsed();

    all_response_times.sort();

    let avg_response_time = if !all_response_times.is_empty() {
        all_response_times.iter().sum::<Duration>() / all_response_times.len() as u32
    } else {
        Duration::from_millis(0)
    };

    let min_response_time = all_response_times
        .first()
        .copied()
        .unwrap_or(Duration::from_millis(0));
    let max_response_time = all_response_times
        .last()
        .copied()
        .unwrap_or(Duration::from_millis(0));

    let p95_index = (all_response_times.len() as f64 * 0.95) as usize;
    let p95_response_time = all_response_times
        .get(p95_index.saturating_sub(1))
        .copied()
        .unwrap_or(Duration::from_millis(0));

    let total_requests = total_successful + total_failed;
    let requests_per_second = total_requests as f64 / actual_duration.as_secs_f64();

    BenchmarkResults {
        total_requests,
        successful_requests: total_successful,
        failed_requests: total_failed,
        avg_response_time,
        min_response_time,
        max_response_time,
        p95_response_time,
        requests_per_second,
        duration: actual_duration,
    }
}

fn print_results(results: &BenchmarkResults) {
    println!("Benchmark Results:");
    println!("==================");
    println!("Total requests: {}", results.total_requests);
    println!("Successful requests: {}", results.successful_requests);
    println!("Failed requests: {}", results.failed_requests);
    println!(
        "Success rate: {:.2}%",
        (results.successful_requests as f64 / results.total_requests as f64) * 100.0
    );
    println!("Requests per second: {:.2}", results.requests_per_second);
    println!("Duration: {:?}", results.duration);
    println!();
    println!("Response Times:");
    println!("  Average: {:?}", results.avg_response_time);
    println!("  Minimum: {:?}", results.min_response_time);
    println!("  Maximum: {:?}", results.max_response_time);
    println!("  95th percentile: {:?}", results.p95_response_time);
}

#[tokio::main]
async fn main() {
    let socket_path = "/tmp/map8x32.sock";

    let test_scenarios = vec![
        (10, Duration::from_secs(10)),
        (50, Duration::from_secs(20)),
        (100, Duration::from_secs(15)),
        (50, Duration::from_secs(10)),
    ];

    for (concurrent_users, duration) in test_scenarios {
        println!(
            "Running test with {} concurrent users for {:?}",
            concurrent_users, duration
        );
        let results = run_load_test(socket_path, concurrent_users, duration).await;
        print_results(&results);
        println!("{}", "=".repeat(50));

        sleep(Duration::from_secs(2)).await;
    }
}
