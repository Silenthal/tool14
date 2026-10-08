use std::io::{Read, Seek};

use anyhow::Result;
use binrw::{BinRead, PosValue, binread};

#[binread]
#[derive(Default, Debug)]
#[br(little, magic = b"CUTB")]
pub struct CutsceneSimple {
    #[br(temp)]
    size: u32,
    #[br(temp)]
    tag_count: u32,
    #[br(count=tag_count)]
    pub tags: Vec<NodeTag>,
}

#[binread]
#[derive(Default, Debug)]
#[br(little)]
pub struct NodeTag {
    pub magic: PosValue<[u8; 4]>,
    #[br(temp)]
    size: u32,
    #[br(map = |off: u32| off as u64 + magic.pos + 8)]
    pub item_offset: u64,
    #[br(temp)]
    item_len: u32,
}

impl CutsceneSimple {
    //bytes: &Vec<u8>
    pub fn from_bytes<R: Read + Seek>(stream: &mut R) -> Result<Self> {
        let cs: CutsceneSimple = CutsceneSimple::read(stream)?;
        Ok(cs)
    }
}
