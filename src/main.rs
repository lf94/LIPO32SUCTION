use std::{
  convert::{
    self,
    TryInto,
  },
  default,
  env,
  fmt,
  fs::File,
  io::{
    self,
    Read,
    Seek,
    SeekFrom,
  },
  mem,
  str,
};

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

#[derive(Debug)]
struct FATHeader {
	bootjmp: [u8; 3],
	oem_name: [u8; 8],
	bytes_per_sector: u16,
	sectors_per_cluster: u8,
	reserved_sector_count: u16,
	table_count: u8,
	root_entry_count: u16,
	total_sectors_16: u16,
	media_type: u8,
	table_size_16: u16,
	sectors_per_track: u16,
	head_side_count: u16,
	hidden_sector_count: u32,
	total_sectors_32: u32,
}

const FATHEADER_SIZE:usize = mem::size_of::<FATHeader>();

#[derive(Debug)]
struct FAT32Ext {
	table_size_32: u32,
	extended_flags: u16,
	fat_version: u16,
	root_cluster: u32,
	fat_info: u16,
	backup_bs_sector: u16,
	reserved_0: [u8; 12],
	drive_number: u8,
	reserved_1: u8,
	boot_signature: u8,
	volume_id: u32,
	volume_label: [u8; 11],
	fat_type_label: [u8; 8],
}

const FAT32EXT_SIZE:usize = mem::size_of::<FAT32Ext>();

impl From<[u8; FATHEADER_SIZE]> for FATHeader {
  fn from(target: [u8; FATHEADER_SIZE]) -> Self {
    FATHeader {
    	bootjmp: target[0..3].try_into().unwrap(),
    	oem_name: target[3..11].try_into().unwrap(),
    	bytes_per_sector: ((target[12] as u16) << 8) | (target[11] as u16),
    	sectors_per_cluster: target[13],
    	reserved_sector_count: ((target[15] as u16) << 8) | (target[14] as u16),
    	table_count: target[16],
    	root_entry_count: ((target[18] as u16) << 8) | (target[17] as u16),
    	total_sectors_16: ((target[20] as u16) << 8) | (target[19] as u16),
    	media_type: target[21],
    	table_size_16: ((target[23] as u16) << 8) | (target[22] as u16),
    	sectors_per_track: ((target[25] as u16) << 8) | (target[24] as u16),
    	head_side_count: ((target[27] as u16) << 8) | (target[26] as u16),
    	hidden_sector_count: ((target[28] as u32) << 24) | ((target[29] as u32) << 16)
        | ((target[30] as u32) << 8) | (target[31] as u32),
    	total_sectors_32: ((target[32] as u32) << 24) | ((target[33] as u32) << 16)
        | ((target[34] as u32) << 8) | (target[35] as u32),
    }
  }
}

impl fmt::Display for FATHeader {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{:?}", self)
  }
}


impl From<[u8; FAT32EXT_SIZE]> for FAT32Ext {
  fn from(target: [u8; FAT32EXT_SIZE]) -> Self {
    FAT32Ext {
    	table_size_32: ((target[3] as u32) << 24) | ((target[2] as u32) << 16)
    	  | ((target[1] as u32) << 8) | (target[0] as u32),
    	extended_flags: ((target[5] as u16) << 8) | (target[4] as u16),
    	fat_version: ((target[7] as u16) << 8) | (target[6] as u16),
    	root_cluster: ((target[11] as u32) << 24) | ((target[10] as u32) << 16)
    	  | ((target[9] as u32) << 8) | (target[8] as u32),
    	fat_info: ((target[13] as u16) << 8) | (target[12] as u16),
    	backup_bs_sector: ((target[15] as u16) << 8) | (target[14] as u16),
    	reserved_0: [0; 12],
    	drive_number: target[28],
    	reserved_1: target[29],
    	boot_signature: target[30],
    	volume_id: ((target[34] as u32) << 24) | ((target[33] as u32) << 16)
    	  | ((target[32] as u32) << 8) | (target[31] as u32),
    	volume_label: target[35..46].try_into().unwrap(),
    	fat_type_label: target[46..54].try_into().unwrap(),
    }
  }
}

impl fmt::Display for FAT32Ext {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{:?}", self)
  }
}

fn main() {
  let args = env::args().collect::<Vec<String>>();
  let mut file = File::open(&args[1]).unwrap();
  let mut fat_header_buf: [u8; FATHEADER_SIZE] = [0; FATHEADER_SIZE];
  file.read(&mut fat_header_buf).unwrap();
  let mut fat32_ext_buf: [u8; FAT32EXT_SIZE] = [0; FAT32EXT_SIZE];
  file.read(&mut fat32_ext_buf).unwrap();
  let fat_header = FATHeader::from(fat_header_buf);
  let fat32_ext = FAT32Ext::from(fat32_ext_buf);

  println!("{}", fat_header);
  println!("{}", fat32_ext);

  /* 
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
  */
}
