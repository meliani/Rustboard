use clap::{Parser, Subcommand};
use serde::Deserialize;

#[derive(Parser)]
#[command(name = "dashboard")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
    #[arg(long, default_value = "http://127.0.0.1:8080")]
    server: String,
}

#[derive(Subcommand)]
enum Commands {
    /// List all services
    List,
    /// Show status for a service
    Status { id: String },
    /// List stored quick commands for a service
    QuickList { id: String },
    /// Execute a stored quick command for a service
    QuickExec { id: String, quick: String },
    /// Start a service
    Start { id: String },
    /// Stop a service
    Stop { id: String },
    /// Restart a service
    Restart { id: String },
    /// Fetch logs for a service
    Logs { id: String, #[arg(short, long, default_value_t = 200)] lines: usize },
    /// Reload config on the server
    ConfigReload,
}

#[derive(Deserialize)]
struct ServiceList { services: Vec<serde_json::Value> }

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let client = reqwest::Client::new();
    match cli.command {
        Commands::List => {
            let resp = client.get(format!("{}/services", cli.server)).send().await?;
            let list: ServiceList = resp.json().await?;
            for s in list.services {
                println!("{} - {}", s.get("id").unwrap_or(&serde_json::Value::String("".into())), s.get("status").unwrap_or(&serde_json::Value::String("".into())));
            }
        }
        Commands::Status { id } => {
            let resp = client.get(format!("{}/services", cli.server)).send().await?;
            let list: ServiceList = resp.json().await?;
            if let Some(s) = list.services.into_iter().find(|v| v.get("id").and_then(|x| x.as_str()) == Some(&id)) {
                println!("{}", serde_json::to_string_pretty(&s)?);
            } else { println!("service not found"); }
        }
        Commands::QuickList { id } => {
            let resp = client.get(format!("{}/services", cli.server)).send().await?;
            let list: ServiceList = resp.json().await?;
            if let Some(s) = list.services.into_iter().find(|v| v.get("id").and_then(|x| x.as_str()) == Some(&id)) {
                if let Some(qs) = s.get("quick_commands").and_then(|q| q.as_array()) {
                    for q in qs {
                        let name = q.get("name").and_then(|n| n.as_str()).unwrap_or("");
                        let desc = q.get("description").and_then(|d| d.as_str()).unwrap_or("");
                        println!("- {}: {}", name, desc);
                    }
                } else { println!("no quick commands configured"); }
            } else { println!("service not found"); }
        }
        Commands::QuickExec { id, quick } => {
            let resp = client.post(format!("{}/services/quick", cli.server))
                .json(&serde_json::json!({"id": id, "quick": quick}))
                .send().await?;
            println!("{}", resp.text().await?);
        }
        Commands::Start { id } => {
            let resp = client.post(format!("{}/services/cmd", cli.server))
                .json(&serde_json::json!({"id": id, "cmd": "start"}))
                .send().await?;
            println!("{}", resp.text().await?);
        }
        Commands::Stop { id } => {
            let resp = client.post(format!("{}/services/cmd", cli.server))
                .json(&serde_json::json!({"id": id, "cmd": "stop"}))
                .send().await?;
            println!("{}", resp.text().await?);
        }
        Commands::Restart { id } => {
            let resp = client.post(format!("{}/services/cmd", cli.server))
                .json(&serde_json::json!({"id": id, "cmd": "restart"}))
                .send().await?;
            println!("{}", resp.text().await?);
        }
        Commands::Logs { id, lines } => {
            let resp = client.post(format!("{}/services/logs", cli.server))
                .json(&serde_json::json!({"id": id, "lines": lines}))
                .send().await?;
            println!("{}", resp.text().await?);
        }
        Commands::ConfigReload => {
            let resp = client.post(format!("{}/config/reload", cli.server)).send().await?;
            println!("{}", resp.text().await?);
        }
    }
    Ok(())
}
