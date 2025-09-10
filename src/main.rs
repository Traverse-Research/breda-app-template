use app_template::CommandlineOpts;
use clap::Parser;

fn main() -> anyhow::Result<()> {
    app_template::internal_main(&CommandlineOpts::parse())
}
