#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(error) = vide_lib::server::run_from_args(args).await {
        eprintln!("vide-server: {error}");
        std::process::exit(1);
    }
}
