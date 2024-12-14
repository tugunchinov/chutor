fn main() {
    chutor::run_with_seed(start(), 666);
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
