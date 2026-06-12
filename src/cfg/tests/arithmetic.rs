use crate::cfg::mir::Ty;
use crate::ctx::ctx::GlobalCtx;
use super::harness::{run_pipeline, compare_or_update};

#[test]
fn test_add_two_args() {
    let mut ctx = GlobalCtx::new();
    let mut b = ctx.create_builder();

    let func = b.add_func("add".to_string(), vec![Ty::I32, Ty::I32], Ty::I32);
    let block = b.append_block(func);
    b.position_at_end(block);

    let x = b.get_nth_param(func, 0);
    let y = b.get_nth_param(func, 1);
    let result = b.build_add(x, y);
    b.build_return_value(result);

    compare_or_update("add_two_args", &run_pipeline(&ctx));
}

#[test]
fn test_sub_two_args() {
    let mut ctx = GlobalCtx::new();
    let mut b = ctx.create_builder();

    let func = b.add_func("sub".to_string(), vec![Ty::I32, Ty::I32], Ty::I32);
    let block = b.append_block(func);
    b.position_at_end(block);

    let x = b.get_nth_param(func, 0);
    let y = b.get_nth_param(func, 1);
    let result = b.build_sub(x, y);
    b.build_return_value(result);

    compare_or_update("sub_two_args", &run_pipeline(&ctx));
}
