//! View layer. Parses arguments with clap and delegates to the feature layer.
//! No domain logic lives here.

pub mod router;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "daneel", version, about)]
pub struct Cli {}
