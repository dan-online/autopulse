use clap_derive::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Location of configuration file
    #[arg(short, long)]
    pub config: Option<String>,
}
