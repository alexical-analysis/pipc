mod cfg;
mod codegen;
mod ctx;
mod isel;
mod regalloc;

use clap::{Parser, Subcommand};

use cfg::mir::Ty;
use codegen::codegen::Gen;
use ctx::ctx::GlobalCtx;
use isel::isel::InstructionSelector;
use regalloc::regalloc::run as regalloc_run;

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

            let _codegen = Gen::new(&ctx);

            let machine_funcs = InstructionSelector::new(&ctx).run();
            println!("instruction selection produced {} function(s)", machine_funcs.len());
            for mut mf in machine_funcs {
                let num_spill_slots = regalloc_run(&mut mf);
                println!(
                    "  func '{}': {} block(s), {} spill slot(s)",
                    mf.name,
                    mf.blocks.len(),
                    num_spill_slots,
                );
                ctx.add_machine_func(mf);
            }
        }
    }
}
