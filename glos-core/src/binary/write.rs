use crate::GLOS_HEADER_SIZE;

pub fn write_u32_local(
    buf: &mut [u8; GLOS_HEADER_SIZE],
    off: &mut usize,
    is_le: bool,
    val: u32,
) {
    if is_le {
        buf[*off..*off + 4].copy_from_slice(&val.to_le_bytes());
    } else {
        buf[*off..*off + 4].copy_from_slice(&val.to_be_bytes());
    }
    *off += 4;
}

pub fn write_u64_local(
    buf: &mut [u8; GLOS_HEADER_SIZE],
    off: &mut usize,
    is_le: bool,
    val: u64,
) {
    if is_le {
        buf[*off..*off + 8].copy_from_slice(&val.to_le_bytes());
    } else {
        buf[*off..*off + 8].copy_from_slice(&val.to_be_bytes());
    }
    *off += 8;
}
