mod emit;
mod isel;
mod link;
mod peephole;
mod prologue;
mod regalloc;

use crate::ctx::ctx::GlobalCtx;

use emit::emit::Emitter;
use isel::isel::InstructionSelector;
use link::link::Linker;
use peephole::peephole::run as peephole_run;
use prologue::prologue::run as prologue_run;
use regalloc::regalloc::run as regalloc_run;

/// Run the full backend pipeline: instruction selection → register allocation →
/// prologue/epilogue insertion → peephole optimization → emission → linking.
/// Returns the flat ROM word stream ready to write to disk.
pub fn compile(ctx: &GlobalCtx) -> Vec<u16> {
    let mut linker = Linker::new();

    for mut mf in InstructionSelector::new(ctx).run() {
        let num_spill_slots = regalloc_run(&mut mf);
        prologue_run(&mut mf, num_spill_slots);
        peephole_run(&mut mf);
        let compiled = Emitter::new(ctx).emit_func(&mf);
        linker.add(compiled);
    }

    linker.link()
}
