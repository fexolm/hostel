use std::io::Error;

use memmap2::{Mmap, MmapMut};

pub struct WritableCodeBuffer {
    mmap: MmapMut,
    len: usize,
}

pub struct ReadonlyCodeBufer {
    mmap: Mmap,
}

impl WritableCodeBuffer {
    pub fn new(len: usize) -> Result<Self, Error> {
        let mmap = MmapMut::map_anon(len)?;
        Ok(Self { mmap, len })
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn finalize(self) -> Result<ReadonlyCodeBufer, Error> {
        let mmap = self.mmap.make_exec()?;
        Ok(ReadonlyCodeBufer { mmap })
    }
}

pub struct CodeWriter<'i> {
    buf: &'i mut WritableCodeBuffer,
    offset: usize,
}

impl<'i> CodeWriter<'i> {
    pub fn new(buf: &'i mut WritableCodeBuffer) -> Self {
        Self { buf, offset: 0 }
    }

    pub fn push(&mut self, bytes: &[u8]) {
        todo!()
    }
}
