use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ptroute", version, about = "PathTraceRoute CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Trace(TraceArgs),
    Build(BuildArgs),
    Render(RenderArgs),
}

#[derive(Args)]
struct TraceArgs {}

#[derive(Args)]
struct BuildArgs {}

#[derive(Args)]
struct RenderArgs {}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Trace(_) => println!("not implemented"),
        Commands::Build(_) => println!("not implemented"),
        Commands::Render(_) => println!("not implemented"),
    }
}
