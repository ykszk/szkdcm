use anyhow::Result;
use szkdcm::Args;
use clap::Parser;

fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();
    szkdcm::main(args)
}