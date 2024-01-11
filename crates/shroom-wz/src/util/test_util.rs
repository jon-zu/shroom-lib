use std::io::{Cursor, Seek};

use binrw::{BinRead, BinReaderExt, BinWrite, BinWriterExt, Endian};

pub fn test_bin_write_read<T: BinWrite + BinRead + std::fmt::Debug + Eq + PartialEq>(
    value: T,
    endian: Endian,
    args_r: <T as BinRead>::Args<'_>,
    args_w: <T as BinWrite>::Args<'_>,
) {
    let mut rw = Cursor::new(Vec::new());
    rw.write_type_args(&value, endian, args_w)
        .expect("failed to write value to buffer");

    rw.rewind().expect("failed to rewind buffer");
    let read_value = rw
        .read_type_args(endian, args_r)
        .expect("failed to read value from buffer");
    assert_eq!(value, read_value);
}

pub fn test_bin_write_read_default<'a, T>(value: T, endian: Endian)
where
    T: BinWrite<Args<'a> = ()> + BinRead<Args<'a> = ()> + std::fmt::Debug + Eq + PartialEq,
{
    test_bin_write_read(value, endian, Default::default(), Default::default());
}
