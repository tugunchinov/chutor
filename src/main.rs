fn main() {
    let executor = chutor::Executor::new();
    executor.block_on(start());
}

async fn start() {
    // our magic business logic goes here
    println!("start!");
    for i in 0..10 {
        chutor::spawn(async move {
            println!("hello from task {i}");
        });
    }
    println!("spawned 10 tasks!");

    std::future::pending::<()>().await
}
