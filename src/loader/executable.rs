use crate::loader::code_buffer::ReadonlyCodeBufer;

pub struct Executable {
    code: ReadonlyCodeBufer,
}

impl Executable {
    fn new(code: ReadonlyCodeBufer) -> Self {
        Self { code }
    }

    pub fn run(&self) {
        todo!()
    }
}
