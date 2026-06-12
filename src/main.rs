mod backend;
mod cfg;
mod ctx;

use clap::{Parser, Subcommand};

use cfg::mir::Ty;
use ctx::ctx::GlobalCtx;

#[derive(Parser)]
#[command(
    name = "pipc",
    about = "Compiler and assembler for the pip16 fantasy console"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Compile or assemble a pip-c / pipasm source file
    Build {
        /// Source file to build
        file: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Build { file: _ } => {
            println!("building, beep boop");

            let mut ctx = GlobalCtx::new();
            let mut builder = ctx.create_builder();

            let func = builder.add_func("sum".to_string(), vec![Ty::I32, Ty::I32], Ty::I32);

            let block = builder.append_block(func);
            builder.position_at_end(block);

            let x = builder.get_nth_param(func, 0);
            let y = builder.get_nth_param(func, 1);

            let sum = builder.build_add(x, y);
            builder.build_return_value(sum);

            let rom = backend::compile(&ctx);
            println!("linked ROM: {} word(s) ({} bytes)", rom.len(), rom.len() * 2);
            for (chunk_idx, chunk) in rom.chunks(8).enumerate() {
                let hex: Vec<String> = chunk.iter().map(|w| format!("{:04X}", w)).collect();
                println!("  {:04X}: {}", chunk_idx * 8, hex.join(" "));
            }
        }
    }
}
