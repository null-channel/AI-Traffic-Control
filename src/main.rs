use clap::{Args, Parser, Subcommand};
use std::net::SocketAddr;
use tracing_subscriber::{fmt, EnvFilter};

mod server;
mod session;
mod settings;
mod discovery;
mod file_ops;
mod git_ops;
use serde_json::json;

#[derive(Debug, Parser)]
#[command(name = "air_traffic_control")] 
#[command(about = "Headless AI coding agent", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Start {
        #[arg(long, default_value = "127.0.0.1:7171")]
        listen: String,
    },
    Session { #[command(subcommand)] cmd: SessionCmd },
    Git { #[command(subcommand)] cmd: GitCmd },
    Discovery { #[command(subcommand)] cmd: DiscoveryCmd },
    Files { #[command(subcommand)] cmd: FilesCmd },
}

#[derive(Debug, Subcommand)]
enum GitCmd {
    Status(RootArg),
    Diff(RootArg),
    AddAll(RootArg),
    Commit(CommitArgs),
}

#[derive(Debug, Subcommand)]
enum SessionCmd {
    Create(SessionCreateArgs),
    List(ServerArg),
    SettingsGet(SessionIdArg),
    SettingsSet(SessionSettingsSetArgs),
    Send(SessionSendArgs),
    Url(SessionUrlArgs),
    Close(SessionIdArg),
}

#[derive(Debug, Subcommand)]
enum DiscoveryCmd {
    List { #[command(flatten)] root: RootArg, #[arg(long, default_value_t = 500)] max: usize },
    Search { #[command(flatten)] root: RootArg, #[arg(long)] pattern: String, #[arg(long, default_value_t = 500)] max: usize },
    Read { #[command(flatten)] root: RootArg, #[arg(long)] path: String, #[arg(long, default_value_t = 65536)] max_bytes: usize },
}

#[derive(Debug, Subcommand)]
enum FilesCmd {
    Write(WriteArgs),
    Move(MoveArgs),
    Delete(DeleteArgs),
}

#[derive(Debug, Args)]
struct RootArg {
    #[arg(long)]
    root: String,
}

#[derive(Debug, Args)]
struct CommitArgs {
    #[command(flatten)]
    root: RootArg,
    #[arg(short, long)]
    message: String,
}

#[derive(Debug, Args)]
struct ServerArg {
    #[arg(long, default_value = "http://127.0.0.1:7171")] 
    server: String,
}

#[derive(Debug, Args)]
struct SessionIdArg {
    #[command(flatten)]
    server: ServerArg,
    #[arg(long)]
    id: String,
}

#[derive(Debug, Args)]
struct SessionCreateArgs {
    #[command(flatten)]
    server: ServerArg,
    #[arg(long)]
    root: Option<String>,
}

#[derive(Debug, Args)]
struct SessionSettingsSetArgs {
    #[command(flatten)]
    id: SessionIdArg,
    #[arg(long)]
    project_root: Option<String>,
    #[arg(long)]
    dry_run: Option<bool>,
    #[arg(long)]
    max_read_bytes: Option<u64>,
}

#[derive(Debug, Args)]
struct SessionSendArgs {
    #[command(flatten)]
    id: SessionIdArg,
    #[arg(long)]
    content: String,
    #[arg(long)]
    model: Option<String>,
}

#[derive(Debug, Args)]
struct SessionUrlArgs {
    #[command(flatten)]
    id: SessionIdArg,
    #[arg(long)]
    url: String,
    #[arg(long, default_value_t = 262144)]
    max_bytes: usize,
}

#[derive(Debug, Args)]
struct WriteArgs {
    #[command(flatten)]
    root: RootArg,
    #[arg(long)]
    path: String,
    #[arg(long)]
    content: Option<String>,
    #[arg(long, value_name = "FILE")] 
    content_file: Option<std::path::PathBuf>,
    #[arg(long, default_value_t = true)]
    create: bool,
    #[arg(long, default_value_t = true)]
    dry_run: bool,
    #[arg(long, default_value_t = 1024)]
    preview_bytes: usize,
}

#[derive(Debug, Args)]
struct MoveArgs {
    #[command(flatten)]
    root: RootArg,
    #[arg(long)]
    from: String,
    #[arg(long)]
    to: String,
    #[arg(long, default_value_t = true)]
    dry_run: bool,
}

#[derive(Debug, Args)]
struct DeleteArgs {
    #[command(flatten)]
    root: RootArg,
    #[arg(long)]
    path: String,
    #[arg(long, default_value_t = true)]
    dry_run: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Start { listen } => {
            let addr: SocketAddr = listen.parse()?;
            let state = server::AppState::default();
            server::serve(addr, state).await?;
        }
        Commands::Session { cmd } => match cmd {
            SessionCmd::Create(args) => {
                let client = reqwest::Client::new();
                let mut body = serde_json::json!({});
                if let Some(root) = args.root {
                    body["settings"] = serde_json::json!({"project_root": root});
                }
                let resp = client.post(format!("{}/v1/sessions", args.server.server))
                    .json(&body)
                    .send()
                    .await?;
                if !resp.status().is_success() {
                    anyhow::bail!("server error: {}", resp.status());
                }
                let v: serde_json::Value = resp.json().await?;
                println!("{}", serde_json::to_string_pretty(&v)?);
            }
            SessionCmd::List(server) => {
                let client = reqwest::Client::new();
                let resp = client.get(format!("{}/v1/sessions", server.server)).send().await?;
                if !resp.status().is_success() { anyhow::bail!("server error: {}", resp.status()); }
                let v: serde_json::Value = resp.json().await?;
                println!("{}", serde_json::to_string_pretty(&v)?);
            }
            SessionCmd::SettingsGet(arg) => {
                let client = reqwest::Client::new();
                let resp = client.get(format!("{}/v1/sessions/{}/settings", arg.server.server, arg.id)).send().await?;
                if !resp.status().is_success() { anyhow::bail!("server error: {}", resp.status()); }
                let v: serde_json::Value = resp.json().await?;
                println!("{}", serde_json::to_string_pretty(&v)?);
            }
            SessionCmd::SettingsSet(args) => {
                let client = reqwest::Client::new();
                let mut patch = serde_json::Map::new();
                if let Some(pr) = args.project_root { patch.insert("project_root".into(), serde_json::Value::from(Some(pr))); }
                if args.dry_run.is_some() || args.max_read_bytes.is_some() {
                    let mut tp = serde_json::Map::new();
                    if let Some(d) = args.dry_run { tp.insert("dry_run".into(), serde_json::Value::from(Some(d))); }
                    if let Some(m) = args.max_read_bytes { tp.insert("max_read_bytes".into(), serde_json::Value::from(Some(m))); }
                    patch.insert("tool_policies".into(), serde_json::Value::Object(tp));
                }
                let resp = client.patch(format!("{}/v1/sessions/{}/settings", args.id.server.server, args.id.id))
                    .json(&patch)
                    .send()
                    .await?;
                if !resp.status().is_success() { anyhow::bail!("server error: {}", resp.status()); }
                let v: serde_json::Value = resp.json().await?;
                println!("{}", serde_json::to_string_pretty(&v)?);
            }
            SessionCmd::Send(args) => {
                let client = reqwest::Client::new();
                let body = serde_json::json!({
                    "role": "user",
                    "content": args.content,
                    "model": args.model,
                });
                let resp = client.post(format!("{}/v1/sessions/{}/messages", args.id.server.server, args.id.id))
                    .json(&body)
                    .send()
                    .await?;
                if !resp.status().is_success() { anyhow::bail!("server error: {}", resp.status()); }
                let v: serde_json::Value = resp.json().await?;
                println!("{}", serde_json::to_string_pretty(&v)?);
            }
            SessionCmd::Url(args) => {
                let client = reqwest::Client::new();
                let body = json!({ "url": args.url, "max_bytes": args.max_bytes });
                let resp = client.post(format!("{}/v1/sessions/{}/context/url", args.id.server.server, args.id.id))
                    .json(&body)
                    .send()
                    .await?;
                if resp.status() == reqwest::StatusCode::FORBIDDEN {
                    anyhow::bail!("host not allowlisted for this session");
                }
                if !resp.status().is_success() { anyhow::bail!("server error: {}", resp.status()); }
                let v: serde_json::Value = resp.json().await?;
                println!("{}", serde_json::to_string_pretty(&v)?);
            }
            SessionCmd::Close(arg) => {
                let client = reqwest::Client::new();
                let resp = client.delete(format!("{}/v1/sessions/{}", arg.server.server, arg.id)).send().await?;
                if resp.status() == reqwest::StatusCode::NO_CONTENT { 
                    println!("{{\"ok\": true}}");
                } else if resp.status() == reqwest::StatusCode::NOT_FOUND {
                    anyhow::bail!("session not found");
                } else {
                    anyhow::bail!("server error: {}", resp.status());
                }
            }
        },
        Commands::Git { cmd } => match cmd {
            GitCmd::Status(RootArg { root }) => {
                let st = git_ops::status(&root)?;
                println!("{}", serde_json::to_string_pretty(&st)?);
            }
            GitCmd::Diff(RootArg { root }) => {
                let diff = git_ops::diff_porcelain(&root)?;
                println!("{}", diff);
            }
            GitCmd::AddAll(RootArg { root }) => {
                git_ops::add_all(&root)?;
                println!("{}", serde_json::json!({"ok": true}));
            }
            GitCmd::Commit(CommitArgs { root: RootArg { root }, message }) => {
                let oid = git_ops::commit(&root, &message)?;
                println!("{}", serde_json::json!({"commit": oid}));
            }
        },
        Commands::Discovery { cmd } => match cmd {
            DiscoveryCmd::List { root: RootArg { root }, max } => {
                let items = discovery::list_files(&root, max);
                println!("{}", serde_json::to_string_pretty(&items)?);
            }
            DiscoveryCmd::Search { root: RootArg { root }, pattern, max } => {
                let items = discovery::search_files(&root, &pattern, max);
                println!("{}", serde_json::to_string_pretty(&items)?);
            }
            DiscoveryCmd::Read { root: RootArg { root }, path, max_bytes } => {
                let content = discovery::read_file_under_root(&root, &path, max_bytes)?;
                println!("{}", serde_json::json!({"path": path, "content": content}));
            }
        },
        Commands::Files { cmd } => match cmd {
            FilesCmd::Write(args) => {
                let content = match (&args.content, &args.content_file) {
                    (Some(s), None) => s.clone(),
                    (None, Some(p)) => std::fs::read_to_string(p)?,
                    _ => anyhow::bail!("provide exactly one of --content or --content-file"),
                };
                let res = file_ops::write_file_under_root(&args.root.root, &args.path, &content, args.create, args.dry_run, args.preview_bytes)?;
                println!("{}", serde_json::to_string_pretty(&res)?);
            }
            FilesCmd::Move(args) => {
                let res = file_ops::move_file_under_root(&args.root.root, &args.from, &args.to, args.dry_run)?;
                println!("{}", serde_json::to_string_pretty(&res)?);
            }
            FilesCmd::Delete(args) => {
                let res = file_ops::delete_file_under_root(&args.root.root, &args.path, args.dry_run)?;
                println!("{}", serde_json::to_string_pretty(&res)?);
            }
        },
    }
    Ok(())
}
