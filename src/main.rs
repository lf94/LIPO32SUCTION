use std::{
  convert::{
    self,
    TryInto,
  },
  default,
  env,
  fs::File,
  io::{
    self,
    Read,
    Seek,
  },
  str,
};

impl convert::From<[u8; 32]> for LongFileName {
  fn from(target: [u8; 32]) -> Self {
    LongFileName {
      order: target[0] & (!0x40 & 0x0F),
      first5chars: target[1..=10].try_into().unwrap(),
      attribute: target[11],
      entry_type: target[12],
      checksum: target[13],
      next6chars: target[14..=25].try_into().unwrap(),
      zeros: target[26..=27].try_into().unwrap(),
      final2chars: target[28..=31].try_into().unwrap(),
    }
  }
}

impl convert::From<[u8; 32]> for Standard8Point3Format {
  fn from(target: [u8; 32]) -> Self {
    Standard8Point3Format {
      filename: target[0..=10].try_into().unwrap(),
      attributes: target[11],
      reserved1: target[12],
      created_tenths_seconds: target[13],
      created_time: target[14..=15].try_into().unwrap(),
      created_date: target[16..=17].try_into().unwrap(),
      last_access_date: target[18..=19].try_into().unwrap(),
      highbits_cluster_number: target[20..=21].try_into().unwrap(),
      last_update_time: target[22..=23].try_into().unwrap(),
      last_update_date: target[24..=25].try_into().unwrap(),
      lowbits_cluster_number: target[26..=27].try_into().unwrap(),
      filesize: target[28..=31].try_into().unwrap(),
    }
  }
}
#[derive(Debug)]
struct Standard8Point3Format {
  filename: [u8; 11],
  attributes: u8,
  reserved1: u8,
  created_tenths_seconds: u8,
  created_time: [u8; 2],
  created_date: [u8; 2],
  last_access_date: [u8; 2],
  highbits_cluster_number: [u8; 2],
  last_update_time: [u8; 2],
  last_update_date: [u8; 2],
  lowbits_cluster_number: [u8; 2],
  filesize: [u8; 4],
}

#[derive(Debug)]
struct LongFileName {
  order: u8,
  first5chars: [u8; 10],
  attribute: u8,
  entry_type: u8,
  checksum: u8,
  next6chars: [u8; 12],
  zeros: [u8; 2],
  final2chars: [u8; 4],
}

#[derive(Debug)]
struct DirEntry {
  long_name: Vec<LongFileName>,
  meta: Standard8Point3Format,
}

impl default::Default for DirEntry {
  fn default() -> Self {
    DirEntry {
      long_name: vec![],
      meta: Standard8Point3Format {
        filename: [0,0,0,0,0,0,0,0,0,0,0],
        attributes: 0,
        reserved1: 0,
        created_tenths_seconds: 0,
        created_time: [0,0],
        created_date: [0,0],
        last_access_date: [0,0],
        highbits_cluster_number: [0,0],
        last_update_time: [0,0],
        last_update_date: [0,0],
        lowbits_cluster_number: [0,0],
        filesize: [0,0,0,0],
      },
    }
  }
}

fn datetime(date: [u8; 2], time: [u8; 2]) -> String {
  let seconds = time[0] & 0x1F;
  let minutes = ((time[1] & 0x07) << 3) | ((time[0] & 0xE0) >> 5);
  let hours = (time[1] & 0xF8) >> 3;

  let day = date[0] & 0x1F;
  let month = ((date[1] & 0x01) << 3) | ((date[0] & 0xE0) >> 5);
  let year = ((date[1] & 0xFE) >> 1) as u16 + 1980;
  let datetime_str = format!("{}-{}-{}T{}:{}:{}", year, month, day, hours, minutes, seconds);
  datetime_str.to_string()
}

impl DirEntry {
  fn name(self: &Self) -> String {
    let mut name_parts = vec![];
    for part in self.long_name.iter() {
      let mut name = vec![];
      name.push(part.first5chars[0]);
      name.push(part.first5chars[2]);
      name.push(part.first5chars[4]);
      name.push(part.first5chars[6]);
      name.push(part.first5chars[8]);
      name.push(part.next6chars[0]);
      name.push(part.next6chars[2]);
      name.push(part.next6chars[4]);
      name.push(part.next6chars[6]);
      name.push(part.next6chars[8]);
      name.push(part.next6chars[10]);
      name.push(part.final2chars[0]);
      name.push(part.final2chars[2]);
      name.append(&mut name_parts);
      name_parts = name;
    }

    String::from_utf8_lossy(&name_parts).to_string()
  }
  fn cluster(self: &Self) -> u32 {
    let hi = self.meta.highbits_cluster_number;
    let lo = self.meta.lowbits_cluster_number;

    let cluster_number =
        ((hi[1] as u32) << 24)
      | ((hi[0] as u32) << 16)
      | ((lo[1] as u32) << 8)
      | ((lo[0] as u32) << 0);

    cluster_number
  }
  fn size(self: &Self) -> u32 {
    let filesize = self.meta.filesize;
    let size =
        ((filesize[3] as u32) << 24)
      | ((filesize[2] as u32) << 16)
      | ((filesize[1] as u32) << 8)
      | ((filesize[0] as u32) << 0);

    size
  }
}

fn main() {
    let args = env::args().collect::<Vec<String>>();
    let mut file = File::open(args[1].clone()).unwrap();
    let dirs_start_offset = args[2].parse::<u64>().unwrap();
    let no_entries = args[3].parse::<u64>().unwrap();

    file.seek(io::SeekFrom::Start(dirs_start_offset)).unwrap();

    let mut dir_entries = vec![];
    let mut buffer = [0; 32];
    let mut current_entry = DirEntry::default();
    
    for _i in 0..no_entries {
      file.read(&mut buffer).unwrap();
      if buffer[0] == 0x00 { break; }
      if buffer[11] == 0x0F {
        let long_name = LongFileName::from(buffer);
        current_entry.long_name.push(long_name);
      } else {
        let normal_entry = Standard8Point3Format::from(buffer);
        current_entry.meta = normal_entry;
        dir_entries.push(current_entry);
        current_entry = DirEntry::default();
      }
    }

    println!(
      "{:?}K {:?} {:?} {:?}",
      dir_entries[0].size() / 1024,
      datetime(dir_entries[0].meta.created_date, dir_entries[0].meta.created_time),
      dir_entries[0].name(),
      dir_entries[0].cluster(),
    );
}
