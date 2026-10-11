//! The `ohrsi-gateway` binary; see [`ohrsi_gateway::server`] for options.

use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = async {
        let options = ohrsi_gateway::server::parse(&args)?;
        let key = std::env::var("OPENROUTER_API_KEY")
            .map_err(|_| ohrsi_gateway::Error::Usage("OPENROUTER_API_KEY is not set".into()))?;
        let bound = ohrsi_gateway::Bound::open(&options, key).await?;
        println!("listening on http://{}", bound.addr);
        bound
            .serve(async {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await
    }
    .await;
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("ohrsi-gateway: {error}");
            ExitCode::FAILURE
        }
    }
}
