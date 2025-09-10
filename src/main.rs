use {{ template_code_id }}::CommandlineOpts;
use clap::Parser;

fn main() -> anyhow::Result<()> {
    {{ template_code_id }}::internal_main(&CommandlineOpts::parse())
}
