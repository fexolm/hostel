pub struct Codegen {}

impl Codegen {
    pub fn new() -> Self {
        Self {}
    }
}

impl super::Codegen for Codegen {
    fn emit_prologue(&mut self, writer: &mut crate::loader::code_buffer::CodeWriter<'_>) {
        todo!()
    }

    fn emit_epilogue(&mut self, writer: &mut crate::loader::code_buffer::CodeWriter<'_>) {
        todo!()
    }

    fn emit_func_call(
        &mut self,
        writer: &mut crate::loader::code_buffer::CodeWriter<'_>,
        func_addr: usize,
    ) {
        todo!()
    }
}
