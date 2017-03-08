// The Computer Language Benchmarks Game
// http://benchmarksgame.alioth.debian.org/
//
// contributed by the Rust Project Developers
// contributed by Cristi Cobzarenco (@cristicbz)
// contributed by TeXitoi
// contributed by Matt Brubeck

extern crate rayon;

use std::io::{BufRead, BufReader, Write};
use std::{cmp, io};
use std::fs::File;
use std::mem::replace;

/// This controls the size of reads from the input. Chosen to match the C entry.
const READ_SIZE: usize = 16 * 1024;

/// Lookup table to find the complement of a single FASTA code.
fn build_table() -> [u8; 256] {
    let mut table = [0; 256];
    for (i, x) in table.iter_mut().enumerate() {
        *x = match i as u8 as char {
            'A' | 'a' => 'T',
            'C' | 'c' => 'G',
            'G' | 'g' => 'C',
            'T' | 't' => 'A',
            'U' | 'u' => 'A',
            'M' | 'm' => 'K',
            'R' | 'r' => 'Y',
            'W' | 'w' => 'W',
            'S' | 's' => 'S',
            'Y' | 'y' => 'R',
            'K' | 'k' => 'M',
            'V' | 'v' => 'B',
            'H' | 'h' => 'D',
            'D' | 'd' => 'H',
            'B' | 'b' => 'V',
            'N' | 'n' => 'N',
            i => i,
        } as u8;
    }
    table
}

/// Utilities for splitting chunks off of slices.
trait SplitOff {
    fn split_off_left(&mut self, n: usize) -> Self;
    fn split_off_right(&mut self, n: usize) -> Self;
}
impl<'a, T> SplitOff for &'a mut [T] {
    /// Split the left `n` items from self and return them as a separate slice.
    fn split_off_left(&mut self, n: usize) -> Self {
        let n = cmp::min(self.len(), n);
        let data = replace(self, &mut []);
        let (left, data) = data.split_at_mut(n);
        *self = data;
        left
    }
    /// Split the right `n` items from self and return them as a separate slice.
    fn split_off_right(&mut self, n: usize) -> Self {
        let len = self.len();
        let n = cmp::min(len, n);
        let data = replace(self, &mut []);
        let (data, right) = data.split_at_mut(len - n);
        *self = data;
        right
    }
}

/// Length of a normal line including the terminating \n.
const LINE_LEN: usize = 61;
const SEQUENTIAL_SIZE: usize = 2048;

/// Compute the reverse complement for two contiguous chunks without line breaks.
fn reverse_chunks(left: &mut [u8], right: &mut [u8], table: &[u8; 256]) {
    for (x, y) in left.iter_mut().zip(right.iter_mut().rev()) {
        *y = table[replace(x, table[*y as usize]) as usize];
    }
}

/// Compute the reverse complement on chunks from opposite ends of a sequence.
///
/// `left` must start at the beginning of a line. If there are an odd number of
/// bytes, `right` will initially be 1 byte longer than `left`; otherwise they
/// will have equal lengths.
fn reverse_complement_left_right(mut left: &mut [u8],
                                 mut right: &mut [u8],
                                 trailing_len: usize,
                                 table: &[u8; 256]) {
    let len = left.len();
    if len <= SEQUENTIAL_SIZE {
        // Each iteration swaps one line from the start of the sequence with one
        // from the end.
        while left.len() > 0  || right.len() > 0 {
            // Get the chunk up to the newline in `right`.
            let mut a = left.split_off_left(trailing_len);
            let mut b = right.split_off_right(trailing_len);
            right.split_off_right(1); // Skip the newline in `right`.

            // If we've reached the middle of the sequence here and there is an
            // odd number of bytes remaining, the odd one will be on the right.
            if b.len() > a.len() {
                let mid = b.split_off_left(1);
                mid[0] = table[mid[0] as usize];
            }

            reverse_chunks(a, b, table);

            // Get the chunk up to the newline in `left`.
            let n = LINE_LEN - 1 - trailing_len;
            a = left.split_off_left(n);
            b = right.split_off_right(n);
            left.split_off_left(1); // Skip the newline in `left`.

            // If we've reached the middle of the sequence and there is an odd
            // number of bytes remaining, the odd one will now be on the left.
            if a.len() > b.len() {
                let mid = a.split_off_right(1);
                mid[0] = table[mid[0] as usize]
            }

            reverse_chunks(a, b, table);
        }
    } else {
        let line_count = len / LINE_LEN;
        let mid = line_count / 2 * LINE_LEN; // Split on a whole number of lines.

        let left1 = left.split_off_left(mid);
        let right1 = right.split_off_right(mid);
        rayon::join(|| reverse_complement_left_right(left,  right,  trailing_len, table),
                    || reverse_complement_left_right(left1, right1, trailing_len, table));
    }
}

/// Compute the reverse complement of one sequence.
fn reverse_complement(seq: &mut [u8], table: &[u8; 256]) {
    let len = seq.len() - 1;
    let seq = &mut seq[..len]; // Drop the last newline
    let trailing_len = len % LINE_LEN;
    let (left, right) = seq.split_at_mut(len / 2);
    reverse_complement_left_right(left, right, trailing_len, table);
}

/// Locate each DNA sequence in the input file and reverse it.
fn split_and_reverse<R>(mut stdin: R,
                        mut buf: Vec<u8>,
                        input_size: usize,
                        table: &[u8; 256]) -> io::Result<Vec<Vec<u8>>>
    where R: BufRead + Send
{
    // Reserve capacity to avoid reallocating.
    buf.reserve(input_size);

    // Read the header line.
    stdin.read_until(b'\n', &mut buf)?;

    // Read the sequence body
    let seq_start = buf.len();
    stdin.read_until(b'>', &mut buf)?;

    let i = buf.len() - 1;
    if buf[i] == b'>' {
        // Found the start of a new sequence. Fork one task to reversing
        // the old sequence, and a second task to read in the new sequence.
        let new_buf = buf.split_off(i);
        let new_size = input_size.saturating_sub(buf.len());

        let (_, seqs) = rayon::join(
            || reverse_complement(&mut buf[seq_start..], table),
            || split_and_reverse(stdin, new_buf, new_size, table));

        let mut result = vec![buf];
        result.append(&mut seqs?);
        return Ok(result)
    }

    // End of file reached.
    if buf.is_empty() {
        return Ok(vec![])
    }
    reverse_complement(&mut buf[seq_start..], table);
    Ok(vec![buf])
}

fn run() -> io::Result<()> {
    let stdin = File::open("/dev/stdin")?;
    let size = stdin.metadata()?.len() as usize;
    let reader = BufReader::with_capacity(READ_SIZE, stdin);

    for seq in split_and_reverse(reader, vec![], size, &build_table())? {
        io::stdout().write_all(&seq)?;
    }
    Ok(())
}

fn main() {
    run().unwrap()
}
