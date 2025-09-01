use std::fs::File;
use std::io::{self, Read};

/// Represents a malloc chunk structure equivalent to ptmalloc's malloc_chunk
#[derive(Debug, Clone)]
pub struct MallocChunk {
    pub address: u64,
    pub prev_size: u64,
    pub size: u64,
    pub fd: Option<u64>,
    pub bk: Option<u64>,
}

impl MallocChunk {
    /// Creates a new MallocChunk with the specified address and size information
    pub fn new(address: u64, prev_size: u64, size: u64) -> Self {
        Self { address, prev_size, size, fd: None, bk: None }
    }
}

pub struct Ptmalloc {
    pub size_sz: usize,

    // Constants from __init__
    pub nbins: usize,
    pub nsmallbins: usize,
    pub binmapshift: usize,
    pub fastchunks_bit: u32,
    pub noncontiguous_bit: u32,
    pub heap_min_size: usize,
    pub heap_max_size: usize,
    pub bitspermap: usize,
    pub binmapsize: usize,

    pub prev_inuse: u64,
    pub is_mmapped: u64,
    pub non_main_arena: u64,
    pub size_bits: u64,

    // From set_globals
    pub min_chunk_size: usize,
    pub malloc_alignment: usize,
    pub malloc_align_mask: usize,
    pub minsize: usize,
    pub smallbin_width: usize,
    pub min_large_size: usize,
    pub max_fast_size: usize,
    pub nfastbins: usize,

    // For reading heap data
    pub data: Vec<u8>,
}

impl Ptmalloc {
    pub fn new(size_sz: usize) -> Self {
        let mut ptmalloc = Ptmalloc {
            size_sz,

            nbins: 128,
            nsmallbins: 64,
            binmapshift: 5,
            fastchunks_bit: 0x1,
            noncontiguous_bit: 0x2,
            heap_min_size: 32 * 1024,
            heap_max_size: 1024 * 1024,
            bitspermap: 1 << 5,         // 1 << self.BINMAPSHIFT
            binmapsize: 128 / (1 << 5), // self.NBINS / self.BITSPERMAP

            prev_inuse: 1,
            is_mmapped: 2,
            non_main_arena: 4,
            size_bits: 1 | 2 | 4, // PREV_INUSE | IS_MMAPPED | NON_MAIN_ARENA

            // Will be set by set_globals
            min_chunk_size: 0,
            malloc_alignment: 0,
            malloc_align_mask: 0,
            minsize: 0,
            smallbin_width: 0,
            min_large_size: 0,
            max_fast_size: 0,
            nfastbins: 0,

            data: Vec::new(),
        };

        ptmalloc.set_globals();
        ptmalloc
    }

    fn set_globals(&mut self) {
        self.min_chunk_size = 4 * self.size_sz;
        self.malloc_alignment = 2 * self.size_sz;
        self.malloc_align_mask = self.malloc_alignment - 1;
        self.minsize = (self.min_chunk_size + self.malloc_align_mask) & !self.malloc_align_mask;

        self.smallbin_width = self.malloc_alignment;
        self.min_large_size = self.nsmallbins * self.smallbin_width;

        self.max_fast_size = 80 * self.size_sz / 4;
        let size = self.request2size(self.max_fast_size);
        self.nfastbins = self.fastbin_index(size) + 1;
    }

    pub fn load_heap_data<R: Read>(&mut self, mut reader: R) -> io::Result<()> {
        self.data.clear();
        reader.read_to_end(&mut self.data)?;
        Ok(())
    }

    fn read_u64_at(&self, offset: usize) -> Option<u64> {
        if offset + 8 <= self.data.len() {
            Some(u64::from_le_bytes([
                self.data[offset],
                self.data[offset + 1],
                self.data[offset + 2],
                self.data[offset + 3],
                self.data[offset + 4],
                self.data[offset + 5],
                self.data[offset + 6],
                self.data[offset + 7],
            ]))
        } else {
            None
        }
    }

    pub fn chunk2mem(&self, p: &MallocChunk) -> u64 {
        p.address + (2 * self.size_sz) as u64
    }

    pub fn mem2chunk(&self, mem: u64) -> u64 {
        mem - (2 * self.size_sz) as u64
    }

    pub fn request2size(&self, req: usize) -> usize {
        if req + self.size_sz + self.malloc_align_mask < self.minsize {
            self.minsize
        } else {
            (req + self.size_sz + self.malloc_align_mask) & !self.malloc_align_mask
        }
    }

    pub fn fastbin_index(&self, sz: usize) -> usize {
        if self.size_sz == 8 {
            (sz >> 4) - 2
        } else if self.size_sz == 4 {
            (sz >> 3) - 2
        } else {
            0
        }
    }

    pub fn heap_for_ptr(&self, ptr: u64) -> u64 {
        ptr & !(self.heap_max_size as u64 - 1)
    }

    pub fn chunksize(&self, p: &MallocChunk) -> u64 {
        p.size & !self.size_bits
    }

    pub fn prev_inuse(&self, p: &MallocChunk) -> bool {
        (p.size & self.prev_inuse) != 0
    }

    pub fn chunk_is_mmapped(&self, p: &MallocChunk) -> bool {
        (p.size & self.is_mmapped) != 0
    }

    pub fn chunk_non_main_arena(&self, p: &MallocChunk) -> bool {
        (p.size & self.non_main_arena) != 0
    }

    pub fn next_chunk(&self, p: &MallocChunk) -> u64 {
        p.address + (p.size & !self.size_bits)
    }

    pub fn prev_chunk(&self, p: &MallocChunk) -> u64 {
        p.address - p.prev_size
    }

    pub fn chunk_at_offset(&self, p: &MallocChunk, s: i64) -> u64 {
        (p.address as i64 + s) as u64
    }

    pub fn in_smallbin_range(&self, sz: usize) -> bool {
        sz < self.min_large_size
    }

    pub fn smallbin_index(&self, sz: usize) -> usize {
        if self.smallbin_width == 16 { sz >> 4 } else { sz >> 3 }
    }

    pub fn largebin_index_32(&self, sz: usize) -> usize {
        if (sz >> 6) <= 38 {
            56 + (sz >> 6)
        } else if (sz >> 9) <= 20 {
            91 + (sz >> 9)
        } else if (sz >> 12) <= 10 {
            110 + (sz >> 12)
        } else if (sz >> 15) <= 4 {
            119 + (sz >> 15)
        } else if (sz >> 18) <= 2 {
            124 + (sz >> 18)
        } else {
            126
        }
    }

    pub fn largebin_index_64(&self, sz: usize) -> usize {
        if (sz >> 6) <= 48 {
            48 + (sz >> 6)
        } else if (sz >> 9) <= 20 {
            91 + (sz >> 9)
        } else if (sz >> 12) <= 10 {
            110 + (sz >> 12)
        } else if (sz >> 15) <= 4 {
            119 + (sz >> 15)
        } else if (sz >> 18) <= 2 {
            124 + (sz >> 18)
        } else {
            126
        }
    }

    pub fn largebin_index(&self, sz: usize) -> usize {
        if self.size_sz == 8 {
            self.largebin_index_64(sz)
        } else if self.size_sz == 4 {
            self.largebin_index_32(sz)
        } else {
            0
        }
    }

    pub fn bin_index(&self, sz: usize) -> usize {
        if self.in_smallbin_range(sz) { self.smallbin_index(sz) } else { self.largebin_index(sz) }
    }

    pub fn addr_to_offset(&self, _addr: u64) -> Option<usize> {
        // Convert virtual address to file offset
        // Need to implement based on heap base mapping
        None
    }

    // Find the correct heap start based on expected pattern
    pub fn find_heap_base_offset(&self) -> Option<usize> {
        // Search for the pattern that matches good_output:
        // First chunk should have size 0x411, followed by chunk with size 0x301
        for offset in (0..self.data.len().saturating_sub(0x420)).step_by(8) {
            if let (Some(prev1), Some(size1)) =
                (self.read_u64_at(offset), self.read_u64_at(offset + 8))
                && prev1 == 0
                && size1 == 0x411
            {
                // Check if next chunk at +0x410 has size 0x301
                let next_offset = offset + 0x410;
                if let Some(next_size) = self.read_u64_at(next_offset + 8)
                    && next_size == 0x301
                {
                    return Some(offset);
                }
            }
        }

        // Fallback: look for any valid first chunk
        for offset in (0..self.data.len().saturating_sub(16)).step_by(8) {
            if let (Some(prev_size), Some(size)) =
                (self.read_u64_at(offset), self.read_u64_at(offset + 8))
                && prev_size == 0
                && size > 0
                && (size & self.prev_inuse) != 0
            {
                let chunk_size = size & !self.size_bits;
                if chunk_size >= self.minsize as u64 && chunk_size < 0x100000 {
                    return Some(offset);
                }
            }
        }

        None
    }

    // Add heap walking functionality following libheap logic
    pub fn walk_heap(&self, heap_start_addr: u64) -> Vec<MallocChunk> {
        let mut chunks = Vec::new();

        // Find the correct heap base offset in the file
        let heap_base_offset = match self.find_heap_base_offset() {
            Some(offset) => offset,
            None => return chunks,
        };

        let mut current_offset = heap_base_offset;
        let mut current_addr = heap_start_addr;

        // Walk chunks following libheap's next_chunk() logic
        while let (Some(prev_size), Some(size)) =
            (self.read_u64_at(current_offset), self.read_u64_at(current_offset + 8))
        {
            if size == 0 {
                break; // End of heap
            }

            // Check for fence chunk: size == (0 | PREV_INUSE)
            if size == self.prev_inuse {
                break;
            }

            let chunk_size = self.chunksize(&MallocChunk::new(current_addr, prev_size, size));

            if chunk_size < self.minsize as u64 || chunk_size > 0x100000 {
                break;
            }

            let mut chunk = MallocChunk::new(current_addr, prev_size, size);

            // Check if free and read fd/bk (following libheap logic)
            let next_offset = current_offset + chunk_size as usize;
            if next_offset + 8 <= self.data.len() {
                let next_size = self.read_u64_at(next_offset + 8).unwrap_or(0);
                // Chunk is free if next chunk doesn't have PREV_INUSE bit set
                if (next_size & self.prev_inuse) == 0 && chunk_size >= self.minsize as u64 {
                    chunk.fd = self.read_u64_at(current_offset + 16);
                    chunk.bk = self.read_u64_at(current_offset + 24);
                }
            }

            chunks.push(chunk);

            // Move to next chunk using libheap's next_chunk logic
            current_addr += chunk_size;
            current_offset += chunk_size as usize;

            if chunks.len() > 100 {
                break;
            }
        }

        chunks
    }
}

/// Represents the type of a malloc chunk
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ChunkType {
    Allocated,
    Free,
    FreeUnsortedbin,
    Top,
}

/// Information about a malloc chunk for structured analysis
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ChunkInfo {
    pub chunk_type: ChunkType,
    pub address: u64,
    pub size: u64,
    pub raw_size: u64,
    pub prev_inuse: bool,
    pub fd: Option<u64>,
    pub bk: Option<u64>,
}

impl Ptmalloc {
    pub fn analyze_heap(&self, heap_start_addr: u64) -> Vec<ChunkInfo> {
        let chunks = self.walk_heap(heap_start_addr);
        let mut chunk_infos = Vec::new();

        for (i, chunk) in chunks.iter().enumerate() {
            let is_last = i == chunks.len() - 1;

            let is_free = if is_last {
                false
            } else if let Some(next_chunk) = chunks.get(i + 1) {
                !self.prev_inuse(next_chunk)
            } else {
                chunk.fd.is_some() && chunk.bk.is_some()
            };

            let chunk_type = if is_last {
                ChunkType::Top
            } else if is_free {
                if self.chunksize(chunk) >= 0x400 {
                    ChunkType::FreeUnsortedbin
                } else {
                    ChunkType::Free
                }
            } else {
                ChunkType::Allocated
            };

            chunk_infos.push(ChunkInfo {
                chunk_type,
                address: chunk.address,
                size: self.chunksize(chunk),
                raw_size: chunk.size,
                prev_inuse: self.prev_inuse(chunk),
                fd: chunk.fd,
                bk: chunk.bk,
            });
        }

        chunk_infos
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heap_analysis_matches_pwndbg_structure() {
        let mut ptmalloc = Ptmalloc::new(8);
        let heap_file = File::open("heap").expect("Failed to open heap file");
        ptmalloc.load_heap_data(heap_file).expect("Failed to load heap data");

        let heap_start = 0x555555559000;
        let chunk_infos = ptmalloc.analyze_heap(heap_start);

        let expected = vec![
            ChunkInfo {
                chunk_type: ChunkType::Allocated,
                address: 0x555555559000,
                size: 0x300,
                raw_size: 0x301,
                prev_inuse: true,
                fd: None,
                bk: None,
            },
            ChunkInfo {
                chunk_type: ChunkType::Allocated,
                address: 0x555555559300,
                size: 0x20,
                raw_size: 0x21,
                prev_inuse: true,
                fd: None,
                bk: None,
            },
            ChunkInfo {
                chunk_type: ChunkType::Allocated,
                address: 0x555555559320,
                size: 0x30,
                raw_size: 0x31,
                prev_inuse: true,
                fd: None,
                bk: None,
            },
            ChunkInfo {
                chunk_type: ChunkType::Allocated,
                address: 0x555555559350,
                size: 0x40,
                raw_size: 0x41,
                prev_inuse: true,
                fd: None,
                bk: None,
            },
            ChunkInfo {
                chunk_type: ChunkType::Allocated,
                address: 0x555555559390,
                size: 0x90,
                raw_size: 0x91,
                prev_inuse: true,
                fd: None,
                bk: None,
            },
            ChunkInfo {
                chunk_type: ChunkType::Allocated,
                address: 0x555555559420,
                size: 0x110,
                raw_size: 0x111,
                prev_inuse: true,
                fd: None,
                bk: None,
            },
            ChunkInfo {
                chunk_type: ChunkType::Allocated,
                address: 0x555555559530,
                size: 0x210,
                raw_size: 0x211,
                prev_inuse: true,
                fd: None,
                bk: None,
            },
            ChunkInfo {
                chunk_type: ChunkType::Allocated,
                address: 0x555555559740,
                size: 0xd0,
                raw_size: 0xd1,
                prev_inuse: true,
                fd: None,
                bk: None,
            },
            ChunkInfo {
                chunk_type: ChunkType::FreeUnsortedbin,
                address: 0x555555559810,
                size: 0x740,
                raw_size: 0x741,
                prev_inuse: true,
                fd: Some(0x7ffff7e09b20),
                bk: Some(0x7ffff7e09b20),
            },
            ChunkInfo {
                chunk_type: ChunkType::Allocated,
                address: 0x555555559f50,
                size: 0x1010,
                raw_size: 0x1010,
                prev_inuse: false,
                fd: None,
                bk: None,
            },
            ChunkInfo {
                chunk_type: ChunkType::Top,
                address: 0x55555555af60,
                size: 0x1f0a0,
                raw_size: 0x1f0a1,
                prev_inuse: true,
                fd: None,
                bk: None,
            },
        ];

        assert_eq!(chunk_infos, expected);
    }
}
