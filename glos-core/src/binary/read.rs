use crate::GLOS_HEADER_SIZE;

pub fn read_u32_local(
    buf: &[u8; GLOS_HEADER_SIZE],
    off: &mut usize,
    is_le: bool,
) -> u32 {
    let b = [buf[*off], buf[*off + 1], buf[*off + 2], buf[*off + 3]];
    *off += 4;
    if is_le {
        u32::from_le_bytes(b)
    } else {
        u32::from_be_bytes(b)
    }
}

pub fn read_u64_local(
    buf: &[u8; GLOS_HEADER_SIZE],
    off: &mut usize,
    is_le: bool,
) -> u64 {
    let b = [
        buf[*off],
        buf[*off + 1],
        buf[*off + 2],
        buf[*off + 3],
        buf[*off + 4],
        buf[*off + 5],
        buf[*off + 6],
        buf[*off + 7],
    ];
    *off += 8;
    if is_le {
        u64::from_le_bytes(b)
    } else {
        u64::from_be_bytes(b)
    }
}
