use anyhow::anyhow;
use binrw::{BinRead, BinResult, Endian, NullString, PosValue, binread, parser};
use std::io::{Read, Seek, SeekFrom};

#[binread]
#[derive(Debug)]
#[br(little)]
pub struct CTTL {
    magic: PosValue<[u8; 4]>,
    size: u32,
    #[br(temp)]
    item_count: u32,
    #[br(args { count: item_count as usize })]
    pub components: Vec<TimelineItem>,
}

/// One item of a timeline.
#[derive(Debug)]
pub enum TimelineItem {
    Command(TLCommand),
    Unknown(TLUnknown),
}

impl BinRead for TimelineItem {
    type Args<'a> = ();

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> binrw::prelude::BinResult<Self> {
        let start = reader.stream_position()?;
        let mut magic: [u8; 4] = [0; 4];
        reader.read_exact(&mut magic)?;
        // let size = u32::read_options(reader, endian, ())?;
        reader.seek(SeekFrom::Start(start))?;
        match &magic {
            [b'C', digits @ ..] if digits.iter().all(u8::is_ascii_digit) => match digits {
                b"098" => {
                    return Ok(Self::Command(TLCommand::parse(reader, endian, ())?));
                }
                _ => {
                    let unk: TLUnknown = TLUnknown::read_options(reader, endian, ())?;
                    return Ok(Self::Unknown(unk));
                }
            },
            _ => {
                let unk: TLUnknown = TLUnknown::read_options(reader, endian, ())?;
                return Ok(Self::Unknown(unk));
            }
        }
    }
}

#[binread]
#[derive(Debug)]
#[br(little)]
pub struct TLCommand {
    magic: PosValue<[u8; 4]>,
    size: u32,
    pub id: i16,
    pub time: i16,
    #[br(ignore)]
    pub kind: TLCommandKind,
}

impl Default for TLCommandKind {
    fn default() -> Self {
        Self::Unknown {
            magic: [0; 4],
            size: 0,
            body: Vec::new(),
        }
    }
}

impl TLCommand {
    #[parser(reader: r, endian)]
    fn parse() -> BinResult<Self> {
        let start = r.stream_position()?;
        let magic = PosValue::<[u8; 4]>::read_options(r, endian, ())?;
        let size = u32::read_options(r, endian, ())?;
        let base = r.stream_position()?;
        let id = i16::read_options(r, endian, ())?;
        let time = i16::read_options(r, endian, ())?;
        let kind = match kind(r, endian, (&magic.val, base))? {
            Some(kind) => kind,
            None => {
                let err = binrw::Error::Custom {
                    pos: start,
                    err: Box::new(anyhow!(
                        "Unknown magic {} passed to TLCommand. Check if this type is suppposed to read this.",
                        String::from_utf8_lossy(&magic.val)
                    )),
                };
                return Err(err);
            }
        };
        Ok(Self {
            magic,
            size,
            id,
            time,
            kind,
        })
    }
}

#[binread]
#[derive(Debug)]
#[br(little)]
pub struct TLUnknown {
    magic: PosValue<[u8; 4]>,
    size: u32,
    #[br(count = size - 8)]
    pub body: Vec<u8>,
}

trait Field: Sized {
    /// `base` is the item's offset base, `item_start + 8`.
    fn read<R: Read + Seek>(reader: &mut R, endian: Endian, base: u64) -> BinResult<Self>;
}

macro_rules! inline {
    ($($type:ty)*) => {
        $(impl Field for $type {
            fn read<R: Read + Seek>(reader: &mut R, endian: Endian, _base: u64) -> BinResult<Self> {
                <$type as BinRead>::read_options(reader, endian, ())
            }
        })*
    };
}

inline!(u8 i16 u16 i32 u32 f32);

macro_rules! commands {
    ($(
        $(#[doc = $doc:literal])+
        $magic:ident { $($(#[$meta:meta])* $vis:vis $field:ident: $type:ty),* $(,)? }
    )*) => {
        $(
            $(#[doc = $doc])+
            #[derive(Debug)]
            pub struct $magic {
                $($(#[$meta])* $vis $field: $type,)*
            }

            impl $magic {
                #[parser(reader: r, endian)]
                fn parse(base: u64) -> BinResult<Self> {
                    Ok(Self {
                        $($field: <$type as Field>::read(r, endian, base)?,)*
                    })
                }
            }
        )*

        #[derive(Debug)]
        pub enum TLCommandKind {
            $($(#[doc = $doc])+ $magic($magic),)*

            /// A magic no reference implementation specifies, as the bytes past the preamble.
            Unknown {
                magic: [u8; 4],
                size: u32,
                body: Vec<u8>,
            },
        }

        #[parser(reader: r, endian)]
        fn kind(magic: &[u8; 4], base: u64) -> BinResult<Option<TLCommandKind>> {
            match magic {
                $(
                    _ if magic.as_slice() == stringify!($magic).as_bytes() => {
                        Ok(Some(TLCommandKind::$magic($magic::parse(r, endian, (base,))?)))
                    }
                )*
                _ => {
                    Ok(None)
                },
            }
        }
    };
}

commands! {
    /// Dialogue choice
    C098 {
        pub unknown_1: u32,
        pub unknown_2: u32,
        pub question: Option<String>,
        pub answers: Vec<String>,
        pub unknown_6: u32
    }
}

impl Field for Option<String> {
    fn read<R: Read + Seek>(reader: &mut R, endian: Endian, base: u64) -> BinResult<Self> {
        let str_offset = u32::read_options(reader, endian, ())?;
        let cur_pos = reader.stream_position()?;
        reader.seek(SeekFrom::Start(base + str_offset as u64))?;
        let temp_str = NullString::read_options(reader, endian, ())?;
        let temp_pos = reader.stream_position()?;
        let read_str = String::from_utf8(temp_str.0).map_err(|e| binrw::Error::Custom {
            pos: temp_pos,
            err: Box::new(e),
        })?;
        reader.seek(SeekFrom::Start(cur_pos))?;
        Ok(Some(read_str))
    }
}

impl Field for Vec<String> {
    fn read<R: Read + Seek>(reader: &mut R, endian: Endian, base: u64) -> BinResult<Self> {
        let answer_list_offset = u32::read_options(reader, endian, ())? as u64 + base;
        let answer_count = u32::read_options(reader, endian, ())?;
        if answer_count == 0 {
            return Ok(Vec::new());
        }
        let cur_pos = reader.stream_position()?;
        reader.seek(SeekFrom::Start(answer_list_offset))?;
        let mut answer_offsets = Vec::new();
        for _ in 0..answer_count {
            answer_offsets.push(u32::read_options(reader, endian, ())?);
        }
        let mut returned = Vec::new();
        for answer_offset in answer_offsets {
            reader.seek(SeekFrom::Start(answer_list_offset + answer_offset as u64))?;
            let temp_pos = reader.stream_position()?;
            let ns = NullString::read_options(reader, endian, ())?;
            let s = String::from_utf8(ns.0).map_err(|e| binrw::Error::Custom {
                pos: temp_pos,
                err: Box::new(e),
            })?;
            returned.push(s);
        }
        reader.seek(SeekFrom::Start(cur_pos))?;
        Ok(returned)
    }
}
