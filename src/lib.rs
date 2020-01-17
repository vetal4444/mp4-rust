extern crate byteorder;

use std::io::prelude::*;
use std::io::{BufReader, Read, SeekFrom};
use std::fs::File;
use std::fmt;
use std::convert::TryInto;
use byteorder::{ReadBytesExt, BigEndian};

const HEADER_SIZE: u32 = 8;

#[derive(Debug)]
pub enum Error {
    InvalidData(&'static str),
}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Default)]
pub struct BMFF {
    ftyp: FtypBox,
    moov: Option<MoovBox>,
}

impl BMFF {
    fn new() -> BMFF {
        Default::default()
    }
}

#[derive(Debug, Default)]
struct BMFFBox {
    head: BoxHeader,
}

impl BMFFBox {
    fn new() -> BMFFBox {
        Default::default()
    }
}

#[derive(Debug, Default)]
struct BoxHeader {
    name: String,
    size: u64,
    offset: u64,
}

#[derive(Debug, Default)]
struct FtypBox {
    major_brand: FourCC,
    minor_version: u32,
    compatible_brands: Vec<FourCC>,
}

#[derive(Debug, Default)]
struct MoovBox {
    mvhd: MvhdBox,
    traks: Vec<TrakBox>,
}

impl MoovBox {
    fn new() -> MoovBox {
        Default::default()
    }
}

#[derive(Debug, Default)]
struct MvhdBox {
    version: u8,
    flags: u32,
    creation_time: u32,
    modification_time: u32,
    timescale: u32,
    duration: u32,
    rate: u32,
}

#[derive(Debug, Default)]
struct TrakBox {
    tkhd: Option<TkhdBox>,
}

impl TrakBox {
    fn new() -> TrakBox {
        Default::default()
    }
}

#[derive(Debug, Default)]
struct TkhdBox {
    version: u8,
    flags: u32,
    creation_time: u32,
    modification_time: u32,
    track_id: u32,
    duration: u64,
    layer:  u16,
    alternate_group: u16,
    volume: u16,
    matrix: Matrix,
    width: u32,
    height: u32,
}

#[derive(Debug, Default)]
struct Matrix {
    a: i32,
    b: i32,
    u: i32,
    c: i32,
    d: i32,
    v: i32,
    x: i32,
    y: i32,
    w: i32,
}


#[derive(Default, PartialEq, Clone)]
pub struct FourCC {
    pub value: String
}

impl From<u32> for FourCC {
    fn from(number: u32) -> FourCC {
        let mut box_chars = Vec::new();
        for x in 0..4 {
            let c = (number >> (x * 8) & 0x0000_00FF) as u8;
            box_chars.push(c);
        }
        box_chars.reverse();

        let box_string = match String::from_utf8(box_chars) {
            Ok(t) => t,
            _ => String::from("null"), // error to retrieve fourcc
        };

        FourCC {
            value: box_string
        }
    }
}

impl fmt::Debug for FourCC {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

pub fn read_mp4(f: File) -> Result<BMFF> {

    // Open file and read boxes.
    let bmff = read_boxes(f).unwrap();

    Ok(bmff)
}

fn read_boxes(f: File) -> Result<BMFF> {
    let filesize = f.metadata().unwrap().len();
    let mut reader = BufReader::new(f);
    let mut bmff = BMFF::new();

    let mut start = 0u64;
    while start < filesize {

        // Get box header.
        let header = read_box_header(&mut reader, start).unwrap();
        let BoxHeader{ name, size, offset } = header;

        let mut b = BMFFBox::new();
        b.head = BoxHeader{
            name: name.try_into().unwrap(),
            size: size as u64,
            offset: offset as u64,
        };

        // Match and parse the filetype.
        match b.head.name.as_ref() {
            "ftyp" => {
                let ftyp = parse_ftyp_box(&mut reader, 0, size as u32).unwrap();
                bmff.ftyp = ftyp;
            }
            "free" => {
                start = 0;
            }
            "mdat" => {
                start = (size as u32 - HEADER_SIZE) as u64;
            }
            "moov" => {
                start = (size as u32 - HEADER_SIZE) as u64;
                let moov = parse_moov_box(&mut reader, 0, size as u32).unwrap();
                bmff.moov = Some(moov);
            }
            "moof" => {
                start = (size as u32 - HEADER_SIZE) as u64;
            }
            _ => break
        };
    }
    Ok(bmff)
}

fn read_box_header(reader: &mut BufReader<File>, start: u64) -> Result<BoxHeader> {
    // Seek to offset.
    let _r = reader.seek(SeekFrom::Current(start as i64));

    // Create and read to buf.
    let mut buf = [0u8;8]; // 8 bytes for box header.
    let _n = reader.read(&mut buf);

    // Get size.
    let s = buf[0..4].try_into().unwrap();
    let size = u32::from_be_bytes(s);

    // TODO: Err if size is 0.
    // if size == 0 { break; }
    
    // Get box type string.
    let t = buf[4..8].try_into().unwrap();
    let typ = match std::str::from_utf8(t) {
        Ok(v) => v,
        Err(e) => panic!("Invalid UTF-8 sequence: {}", e),
    };

    let offset = match size {
        1 => 4 + 4 + 8,
        _ => 4 + 4,
    };
    Ok(BoxHeader {
        name: typ.try_into().unwrap(),
        size: size as u64,
        offset: offset as u64,
    })
}

fn parse_ftyp_box(f: &mut BufReader<File>, _offset: u64, size: u32) -> Result<FtypBox> {
    let major = f.read_u32::<byteorder::BigEndian>().unwrap();
    let minor = f.read_u32::<byteorder::BigEndian>().unwrap();
    if size % 4 != 0 {
        return Err(Error::InvalidData("invalid ftyp size"));
    }
    let brand_count = (size - 16) / 4; // header + major + minor

    let mut brands = Vec::new();
    for _ in 0..brand_count {
        let b = f.read_u32::<byteorder::BigEndian>().unwrap();
        brands.push(From::from(b));
    }

    Ok(FtypBox {
        major_brand: From::from(major),
        minor_version: minor,
        compatible_brands: brands,
    })
}

fn parse_moov_box(f: &mut BufReader<File>, _offset: u64, size: u32) -> Result<MoovBox> {
    let mut moov = MoovBox::new();

    let mut start = 0u64;
    while start < size as u64 {

        // Get box header.
        let header = read_box_header(f, start).unwrap();
        let BoxHeader{ name, size: s, offset } = header;

        let mut b = BMFFBox::new();
        b.head = BoxHeader{
            name: name.try_into().unwrap(),
            size: s as u64,
            offset: offset as u64,
        };


        match b.head.name.as_ref() {
            "mvhd" => {
                moov.mvhd = parse_mvhd_box(f, 0, s as u32).unwrap();
            }
            "trak" => {
                let trak = parse_trak_box(f, 0, s as u32).unwrap();
                moov.traks.push(trak);
                // start = (s as u32 - HEADER_SIZE) as u64;
            }
            "udta" => {
                println!("found udta");
                start = (s as u32 - HEADER_SIZE) as u64;
            }
            _ => break
        }
    }
    Ok(moov)
}

fn parse_mvhd_box(f: &mut BufReader<File>, _offset: u64, size: u32) -> Result<MvhdBox> {
    let current =  f.seek(SeekFrom::Current(0)).unwrap(); // Current cursor position.

    let version = f.read_u8().unwrap();
    let flags_a = f.read_u8().unwrap();
    let flags_b = f.read_u8().unwrap();
    let flags_c = f.read_u8().unwrap();
    let flags = u32::from(flags_a) << 16 | u32::from(flags_b) << 8 | u32::from(flags_c);
    let creation_time = f.read_u32::<byteorder::BigEndian>().unwrap();
    let modification_time = f.read_u32::<byteorder::BigEndian>().unwrap();
    let timescale = f.read_u32::<byteorder::BigEndian>().unwrap();
    let duration = f.read_u32::<byteorder::BigEndian>().unwrap();
    let rate = f.read_u32::<byteorder::BigEndian>().unwrap();

    // Skip remaining bytes.
    let after =  f.seek(SeekFrom::Current(0)).unwrap();
    let remaining_bytes = (size as u64 - (after - current)) as i64;
    f.seek(SeekFrom::Current(remaining_bytes - HEADER_SIZE as i64)).unwrap();

    Ok(MvhdBox{
        version,
        flags,
        creation_time,
        modification_time,
        timescale,
        duration,
        rate,
    })
}

fn parse_trak_box(f: &mut BufReader<File>, _offset: u64, size: u32) -> Result<TrakBox> {
    let current =  f.seek(SeekFrom::Current(0)).unwrap(); // Current cursor position.
    let mut trak = TrakBox::new();

    let mut start = 0u64;
    while start < size as u64 {
        // Get box header.
         let header = read_box_header(f, start).unwrap();
         let BoxHeader{ name, size: s, offset } = header;

         let mut b = BMFFBox::new();
         b.head = BoxHeader{
             name: name.try_into().unwrap(),
             size: s as u64,
             offset: offset as u64,
         };

         match b.head.name.as_ref() {
             "tkhd" => {
                 println!("found tkhd");
                 let tkhd = parse_tkhd_box(f, 0, s as u32).unwrap();
                 trak.tkhd = Some(tkhd);
                 start = (s as u32 - HEADER_SIZE) as u64;
             }
             "edts" => {
                 println!("found edts");
                 start = (s as u32 - HEADER_SIZE) as u64;
             }
             "mdia" => {
                 println!("found mdia");
                 start = (s as u32 - HEADER_SIZE) as u64;
             }
             _ => break
         }
    }

    // Skip remaining bytes.
    let after =  f.seek(SeekFrom::Current(0)).unwrap();
    let remaining_bytes = (size as u64 - (after - current)) as i64;
    f.seek(SeekFrom::Current(remaining_bytes - HEADER_SIZE as i64)).unwrap();
    Ok(trak)
}


fn parse_tkhd_box(f: &mut BufReader<File>, _offset: u64, size: u32) -> Result<TkhdBox> {
    let current =  f.seek(SeekFrom::Current(0)).unwrap(); // Current cursor position.

    let version = f.read_u8().unwrap();
    let flags_a = f.read_u8().unwrap();
    let flags_b = f.read_u8().unwrap();
    let flags_c = f.read_u8().unwrap();
    let flags = u32::from(flags_a) << 16 | u32::from(flags_b) << 8 | u32::from(flags_c);
    let creation_time = f.read_u32::<byteorder::BigEndian>().unwrap();
    let modification_time = f.read_u32::<byteorder::BigEndian>().unwrap();
    let track_id = f.read_u32::<byteorder::BigEndian>().unwrap();
    let duration = f.read_u64::<byteorder::BigEndian>().unwrap();
    f.read_u64::<byteorder::BigEndian>().unwrap(); // skip.
    let layer = f.read_u16::<byteorder::BigEndian>().unwrap();
    let alternate_group = f.read_u16::<byteorder::BigEndian>().unwrap();
    let volume = f.read_u16::<byteorder::BigEndian>().unwrap() >> 8;

    f.read_u8().unwrap(); // skip.
    let matrix = Matrix{
        a: f.read_i32::<byteorder::LittleEndian>().unwrap(),
        b: f.read_i32::<byteorder::BigEndian>().unwrap(),
        u: f.read_i32::<byteorder::BigEndian>().unwrap(),
        c: f.read_i32::<byteorder::BigEndian>().unwrap(),
        d: f.read_i32::<byteorder::BigEndian>().unwrap(),
        v: f.read_i32::<byteorder::BigEndian>().unwrap(),
        x: f.read_i32::<byteorder::BigEndian>().unwrap(),
        y: f.read_i32::<byteorder::BigEndian>().unwrap(),
        w: f.read_i32::<byteorder::BigEndian>().unwrap(),
    };

    let width = f.read_u32::<byteorder::BigEndian>().unwrap() >> 8;
    let height = f.read_u32::<byteorder::BigEndian>().unwrap() >> 8;

    // Skip remaining bytes.
    let after =  f.seek(SeekFrom::Current(0)).unwrap();
    let remaining_bytes = (size as u64 - (after - current)) as i64;
    f.seek(SeekFrom::Current(remaining_bytes - HEADER_SIZE as i64)).unwrap();

    Ok(TkhdBox {
        version,
        flags,
        creation_time,
        modification_time,
        track_id,
        duration,
        layer,
        alternate_group,
        volume,
        matrix,
        width,
        height,
    })
}
