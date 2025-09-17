use {{ template_code_friendly_id }}::CommandlineOpts;
use clap::Parser;

fn main() -> anyhow::Result<()> {
    {{ template_code_friendly_id }}::internal_main(&CommandlineOpts::parse())
}
