// The Computer Language Benchmarks Game
// http://benchmarksgame.alioth.debian.org/
//
// contributed by the Rust Project Developers
// contributed by Matt Brubeck
// contributed by Cristi Cobzarenco (@cristicbz)
// contributed by TeXitoi

extern crate rayon;
extern crate memchr;

use std::io::{Read, Write};
use std::{cmp, io, mem, slice};
use std::fs::File;

struct Tables {
    table8: [u8;1 << 8],
    table16: [u16;1 << 16]
}

impl Tables {
    fn new() -> Tables {
        let mut table8 = [0;1 << 8];
        for (i, v) in table8.iter_mut().enumerate() {
            *v = Tables::computed_cpl8(i as u8);
        }
        let mut table16 = [0;1 << 16];
        for (i, v) in table16.iter_mut().enumerate() {
            *v = (table8[i & 255] as u16) << 8 |
                 table8[i >> 8]  as u16;
        }
        Tables { table8: table8, table16: table16 }
    }

    fn computed_cpl8(c: u8) -> u8 {
        match c {
            b'A' | b'a' => b'T',
            b'C' | b'c' => b'G',
            b'G' | b'g' => b'C',
            b'T' | b't' => b'A',
            b'U' | b'u' => b'A',
            b'M' | b'm' => b'K',
            b'R' | b'r' => b'Y',
            b'W' | b'w' => b'W',
            b'S' | b's' => b'S',
            b'Y' | b'y' => b'R',
            b'K' | b'k' => b'M',
            b'V' | b'v' => b'B',
            b'H' | b'h' => b'D',
            b'D' | b'd' => b'H',
            b'B' | b'b' => b'V',
            b'N' | b'n' => b'N',
            i => i,
        }
    }

    /// Retrieves the complement for `i`.
    fn cpl8(&self, i: u8) -> u8 {
        self.table8[i as usize]
    }

    /// Retrieves the complement for `i`.
    fn cpl16(&self, i: u16) -> u16 {
        self.table16[i as usize]
    }
}

trait SliceUtils<'a> {
    fn as_u16_slice(self) -> &'a mut [u16];
    fn split_off_left(&mut self, n: usize) -> Self;
    fn split_off_right(&mut self, n: usize) -> Self;
}
impl<'a> SliceUtils<'a> for &'a mut [u8] {
    fn as_u16_slice(self) -> &'a mut [u16] {
        unsafe { slice::from_raw_parts_mut(self.as_mut_ptr() as *mut u16, self.len() / 2) }
    }
    /// Split the left `n` items from self and return them as a separate slice.
    fn split_off_left(&mut self, n: usize) -> Self {
        let n = cmp::min(self.len(), n);
        let data = mem::replace(self, &mut []);
        let (left, data) = data.split_at_mut(n);
        *self = data;
        left
    }
    /// Split the right `n` items from self and return them as a separate slice.
    fn split_off_right(&mut self, n: usize) -> Self {
        let len = self.len();
        let n = cmp::min(len, n);
        let data = mem::replace(self, &mut []);
        let (data, right) = data.split_at_mut(len - n);
        *self = data;
        right
    }
}

/// Length of a normal line including the terminating \n.
const LINE_LEN: usize = 61;
const SEQUENTIAL_SIZE: usize = 2048;

/// Compute the reverse complement for two contiguous chunks without line breaks.
fn reverse_complement_chunk(mut left: &mut [u8], mut right: &mut [u8], tables: &Tables) {
    // Convert to [u16] to  two bytes at a time.
    let u16_len = (left.len() / 2) * 2;
    let left16 = left.split_off_left(u16_len).as_u16_slice();
    let right16 = right.split_off_right(u16_len).as_u16_slice();
    for (x, y) in left16.iter_mut().zip(right16.iter_mut().rev()) {
        let tmp = tables.cpl16(*x);
        *x = tables.cpl16(*y);
        *y = tmp;
    }

    // If there were an odd number of bytes per slice, handle the remaining single bytes.
    if let (Some(x), Some(y)) = (left.first_mut(), right.first_mut()) {
        let tmp = tables.cpl8(*x);
        *x = tables.cpl8(*y);
        *y = tmp;
    }
}

/// Compute the reverse complement on chunks from opposite ends of the sequence.
///
/// `left` must start at the beginning of a line.
fn reverse_complement_left_right(mut left: &mut [u8], mut right: &mut [u8], trailing_len: usize, tables: &Tables) {
    let len = left.len();
    if len <= SEQUENTIAL_SIZE {
        while left.len() > 0  || right.len() > 0 {
            // Process the chunk up to the newline in `right`.
            let mut a = left.split_off_left(trailing_len);
            let mut b = right.split_off_right(trailing_len);

            // If there is an odd number of bytes, the extra one will be on the right.
            if b.len() > a.len() {
                let mid = b.split_off_left(1);
                mid[0] = tables.cpl8(mid[0])
            }
            reverse_complement_chunk(a, b, tables);

            // Skip the newline in `right`.
            right.split_off_right(1);

            // Process the chunk up to the newline in `left`.
            let leading_len = LINE_LEN - 1 - trailing_len;
            a = left.split_off_left(leading_len);
            b = right.split_off_right(leading_len);

            // If there is an odd number of bytes, the extra one will be on the left.
            if a.len() > b.len() {
                let mid = a.split_off_right(1);
                mid[0] = tables.cpl8(mid[0])
            }
            reverse_complement_chunk(a, b, tables);

            // Skip the newline in `left`.
            left.split_off_left(1);
        }
    } else {
        let line_count = (len + LINE_LEN - 1) / LINE_LEN;
        let mid = line_count / 2 * LINE_LEN; // Split on a whole number of lines.

        let left1 = left.split_off_left(mid);
        let right1 = right.split_off_right(mid);
        rayon::join(|| reverse_complement_left_right(left,  right,  trailing_len, tables),
                    || reverse_complement_left_right(left1, right1, trailing_len, tables));
    }
}

/// Compute the reverse complement.
fn reverse_complement(seq: &mut [u8], tables: &Tables) {
    let len = seq.len() - 1;
    let seq = &mut seq[..len]; // Drop the last newline
    let trailing_len = len % LINE_LEN;
    let (left, right) = seq.split_at_mut(len / 2);
    reverse_complement_left_right(left, right, trailing_len, tables);
}

fn file_size(f: &mut File) -> io::Result<usize> {
    Ok(f.metadata()?.len() as usize)
}

fn split_and_reverse<'a>(data: &mut [u8], tables: &Tables) {
    let data = match memchr::memchr(b'\n', data) {
        Some(i) => &mut data[i + 1..],
        None => return,
    };

    match memchr::memchr(b'>', data) {
        Some(i) => {
            let (head, tail) = data.split_at_mut(i);
            rayon::join(|| reverse_complement(head, tables),
                        || split_and_reverse(tail, tables));
        }
        None => reverse_complement(data, tables),
    };
}

fn main() {
    let mut stdin = File::open("/dev/stdin").expect("Could not open /dev/stdin");
    let size = file_size(&mut stdin).unwrap_or(1024 * 1024);
    let mut data = Vec::with_capacity(size + 1);
    stdin.read_to_end(&mut data).unwrap();
    let tables = &Tables::new();

    split_and_reverse(&mut data, tables);
    let stdout = io::stdout();
    stdout.lock().write_all(&data).unwrap();
}
