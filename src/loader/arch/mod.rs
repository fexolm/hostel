use crate::loader::code_buffer::CodeWriter;

#[cfg(target_arch = "x86_64")]
mod x64;

pub trait Codegen {
    fn emit_prologue(&mut self, writer: &mut CodeWriter<'_>);
    fn emit_epilogue(&mut self, writer: &mut CodeWriter<'_>);
    fn emit_func_call(&mut self, writer: &mut CodeWriter<'_>, func_addr: usize);
}

pub fn get_target_codegen() -> impl Codegen {
    #[cfg(target_arch = "x86_64")]
    return x64::Codegen::new();
}
