const MALLOC_ALIGNMENT: usize = 16;
const SIZE_SZ: usize = size_of::<usize>();
const MALLOC_ALIGN_MASK: usize = MALLOC_ALIGNMENT - 1;
const CHUNK_SIZE_MASK: usize = !(MALLOC_ALIGN_MASK);
const PREV_INUSE: usize = 0x1;
const IS_MMAPPED: usize = 0x2;
const NON_MAIN_ARENA: usize = 0x4;

// Glibc malloc_chunk structure
#[derive(Debug, Copy, Clone)]
pub struct MallocChunk {
    pub prev_size: usize,
    pub size: usize,
    pub data_start_offset: usize,
    pub raw_size: usize,
}

impl MallocChunk {
    pub fn actual_size(&self) -> usize {
        self.size & CHUNK_SIZE_MASK
    }

    pub fn is_in_use(&self) -> bool {
        (self.raw_size & PREV_INUSE) != 0
    }

    pub fn is_mmapped(&self) -> bool {
        (self.raw_size & IS_MMAPPED) != 0
    }

    pub fn is_non_main_arena(&self) -> bool {
        (self.raw_size & NON_MAIN_ARENA) != 0
    }

    pub fn data_size(&self) -> usize {
        if self.actual_size() < 2 * SIZE_SZ {
            0
        } else {
            self.actual_size() - 2 * SIZE_SZ
        }
    }
}

// Structure to represent a free chunk with forward/backward pointers
#[derive(Debug)]
pub struct FreeChunk {
    chunk: MallocChunk,
    fd: usize,
    bk: usize,
}

// Structure for the heap dump analysis
#[derive(Debug)]
pub struct HeapDump {
    pub chunks: Vec<MallocChunk>,
    pub free_chunks: Vec<FreeChunk>,
    pub total_size: usize,
    pub total_allocated: usize,
    pub total_free: usize,
}

pub fn parse_heap(data: &[u8]) -> HeapDump {
    let mut chunks = Vec::new();
    let mut free_chunks = Vec::new();
    let mut offset = 0;
    let mut total_allocated = 0;
    let mut total_free = 0;

    while offset + 2 * SIZE_SZ <= data.len() {
        let prev_size = read_usize(&data[offset..]);
        let raw_size = read_usize(&data[offset + SIZE_SZ..]);
        let size = raw_size & CHUNK_SIZE_MASK;

        if size < 2 * SIZE_SZ || offset + size > data.len() {
            break;
        }

        let chunk =
            MallocChunk { prev_size, size, data_start_offset: offset + 2 * SIZE_SZ, raw_size };

        if !chunk.is_in_use() && offset + 4 * SIZE_SZ <= data.len() {
            let fd = read_usize(&data[offset + 2 * SIZE_SZ..]);
            let bk = read_usize(&data[offset + 3 * SIZE_SZ..]);

            let free_chunk = FreeChunk { chunk: chunk.clone(), fd, bk };

            free_chunks.push(free_chunk);
            total_free += chunk.data_size();
        } else if chunk.is_in_use() {
            total_allocated += chunk.data_size();
        }

        chunks.push(chunk);

        offset += size;
    }

    HeapDump { chunks, free_chunks, total_size: offset, total_allocated, total_free }
}

fn read_usize(data: &[u8]) -> usize {
    if data.len() < SIZE_SZ {
        return 0;
    }

    let mut value: usize = 0;
    for i in 0..SIZE_SZ {
        value |= (data[i] as usize) << (i * 8);
    }
    value
}
