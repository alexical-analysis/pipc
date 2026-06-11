mod cfg;
mod codegen;
mod ctx;
mod emit;
mod isel;
mod peephole;
mod prologue;
mod regalloc;

use clap::{Parser, Subcommand};

use cfg::mir::Ty;
use codegen::codegen::Gen;
use ctx::ctx::GlobalCtx;
use emit::emit::Emitter;
use isel::isel::InstructionSelector;
use peephole::peephole::run as peephole_run;
use prologue::prologue::run as prologue_run;
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
                prologue_run(&mut mf, num_spill_slots);
                let before = mf.blocks.iter().map(|b| b.insts.len()).sum::<usize>();
                peephole_run(&mut mf);
                let after = mf.blocks.iter().map(|b| b.insts.len()).sum::<usize>();

                // Emitter is constructed inline so it drops (releasing &ctx)
                // before the mutable add_machine_func call below.
                let compiled = Emitter::new(&ctx).emit_func(&mf);
                println!(
                    "  func '{}': {} inst(s) ({} eliminated) → {} word(s), {} relocation(s)",
                    mf.name, after, before - after,
                    compiled.words.len(), compiled.relocations.len(),
                );
                for (i, &word) in compiled.words.iter().enumerate() {
                    println!("    {:04X}: {:04X}", i, word);
                }

                ctx.add_machine_func(mf);
            }
        }
    }
}
