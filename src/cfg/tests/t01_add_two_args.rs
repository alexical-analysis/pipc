use crate::cfg::mir::Ty;
use crate::ctx::ctx::GlobalCtx;
use super::harness::{run_pipeline, compare_or_update};

#[test]
fn t01_add_two_args() {
    let mut ctx = GlobalCtx::new();
    let mut b = ctx.create_builder();

    let func = b.add_func("add".to_string(), vec![Ty::I32, Ty::I32], Ty::I32);
    let block = b.append_block(func);
    b.position_at_end(block);

    let x = b.get_nth_param(func, 0);
    let y = b.get_nth_param(func, 1);
    let result = b.build_add(x, y);
    b.build_return_value(result);

    let rom = run_pipeline(&ctx);
    compare_or_update("t01_add_two_args", &rom);
}
