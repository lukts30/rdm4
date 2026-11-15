//! Same-endian model extraction using validated source ownership.

use std::{
    error::Error,
    io::{Read, Seek, SeekFrom, Write},
    ops::Range,
};

use super::{RdmBlock, RdmDocument, RdmHeader, RdmReader, RdmSourceLayout};

/// Copies one model and the shared file records, relocating known references.
/// Opaque tails and numeric payloads retain their original bytes and byte order.
pub struct RdmSplitter<R> {
    source: R,
    layout: RdmSourceLayout,
    models: usize,
    parents: Vec<Option<usize>>,
    children: Vec<Vec<usize>>,
}

struct Placement {
    payload: Range<usize>,
    destination: u32,
}

impl<R: Read + Seek> RdmSplitter<R> {
    /// Validate the complete record tree before allowing extraction.
    pub fn new(mut source: R) -> Result<Self, Box<dyn Error>> {
        let decoded = RdmReader::new(&mut source)?.read_root::<RdmDocument>()?;
        let layout = decoded.layout;
        // The root's second field is its model-table reference.
        let models = layout
            .blocks
            .iter()
            .position(|block| block.reference_offset == 0x20)
            .ok_or("input contains no models")?;
        let mut parents = vec![None; layout.blocks.len()];
        let mut children = vec![Vec::new(); layout.blocks.len()];
        for (i, block) in layout.blocks.iter().enumerate().skip(1) {
            let parent = layout
                .blocks
                .partition_point(|candidate| {
                    candidate.source.payload.start <= block.reference_offset
                })
                .checked_sub(1)
                .ok_or("reference has no parent block")?;
            if parent >= i || block.reference_offset + 4 > layout.blocks[parent].source.payload.end
            {
                return Err("reference is outside its parent block".into());
            }
            parents[i] = Some(parent);
            children[parent].push(i);
        }
        Ok(Self {
            source,
            layout,
            models,
            parents,
            children,
        })
    }

    /// Number of model records available for extraction.
    pub fn model_count(&self) -> usize {
        self.layout.blocks[self.models].source.count as usize
    }

    fn plan(&self, index: usize) -> Result<Vec<Option<Placement>>, Box<dyn Error>> {
        if index >= self.model_count() {
            return Err(format!(
                "model index {index} out of range for {} models",
                self.model_count()
            )
            .into());
        }
        let models = &self.layout.blocks[self.models].source;
        let start = models.payload.start + index * models.stride as usize;
        let selected = start..start + models.stride as usize;
        let mut plan: Vec<Option<Placement>> = Vec::with_capacity(self.layout.blocks.len());
        let mut next = RdmHeader::SIZE;
        for (i, block) in self.layout.blocks.iter().enumerate() {
            let keep = match self.parents[i] {
                None => true,
                Some(parent) => plan[parent]
                    .as_ref()
                    .is_some_and(|placement| placement.payload.contains(&block.reference_offset)),
            };
            if keep {
                let payload = if i == self.models {
                    selected.clone()
                } else {
                    block.source.payload.clone()
                };
                let destination = next
                    .checked_add(RdmBlock::HEADER_SIZE)
                    .ok_or("split offset overflow")?;
                next = destination
                    .checked_add(payload.len())
                    .ok_or("split size overflow")?;
                plan.push(Some(Placement {
                    payload,
                    destination: u32::try_from(destination)?,
                }));
            } else {
                plan.push(None);
            }
        }
        Ok(plan)
    }

    /// Write one single-model RDM from byte zero of a fresh output stream.
    /// All shared file records are retained; only other model subtrees are removed.
    pub fn write_model(
        &mut self,
        index: usize,
        mut output: impl Write,
    ) -> Result<(), Box<dyn Error>> {
        let plan = self.plan(index)?;
        let endian = self.layout.endian;
        output.write_all(&RdmHeader { endian }.encode())?;
        for (i, placement) in plan.iter().enumerate() {
            let Some(placement) = placement else { continue };
            let source = &self.layout.blocks[i].source;
            let mut header = [0; RdmBlock::HEADER_SIZE];
            let count = if i == self.models { 1 } else { source.count };
            endian.encode(count, &mut header[..4]);
            endian.encode(source.stride, &mut header[4..]);
            let mut payload = vec![0; placement.payload.len()];
            self.source
                .seek(SeekFrom::Start(placement.payload.start as u64))?;
            self.source.read_exact(&mut payload)?;
            for &child in &self.children[i] {
                if let Some(child_placement) = &plan[child] {
                    let at = self.layout.blocks[child].reference_offset - placement.payload.start;
                    endian.encode(child_placement.destination, &mut payload[at..at + 4]);
                }
            }
            output.write_all(&header)?;
            output.write_all(&payload)?;
        }
        Ok(())
    }
}
