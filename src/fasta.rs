// The Computer Language Benchmarks Game
// http://benchmarksgame.alioth.debian.org/
//
// contributed by Matt Brubeck

use std::io::{self, Write, BufWriter};

const LINE_LENGTH: usize = 60;
const BLOCK_SIZE: usize = LINE_LENGTH * 1024;
const IM: u32 = 139968;

/// Pseudo-random number generator
struct Rng(u32);
impl Rng {
    fn new() -> Self { Rng(42) }

    fn gen(&mut self, probabilities: &[(u32, u8)], buf: &mut [u8]) {
        for i in buf.iter_mut() {
            self.0 = (self.0 * 3877 + 29573) % IM;
            *i = probabilities.iter().find(|&&(p, _)| p >= self.0).unwrap().1;
        }
    }
}

/// From a probability distribution, generate a cumulative probability distribution.
fn cumulative_probabilities(data: &[(char, f32)]) -> Vec<(u32, u8)> {
    data.iter().scan(0., |sum, &(ch, p)| {
        *sum += p;
        Some(((*sum * IM as f32).floor() as u32, ch as u8))
    }).collect()
}

/// Output FASTA data from the provided generator function.
fn make_fasta<F, W>(n: usize, mut f: F, w: &mut W)
    where F: FnMut(&mut [u8]), W: Write
{
    let mut block = vec![0; BLOCK_SIZE];

    // Write whole blocks.
    let num_blocks = n / BLOCK_SIZE;
    for _ in 0..num_blocks {
        f(&mut block);
        write(&block, w).unwrap();
    }

    // Write trailing block.
    let trailing_len = n % BLOCK_SIZE;
    if trailing_len > 0 {
        f(&mut block[..trailing_len]);
        write(&block[..trailing_len], w).unwrap();
    }
}

/// Print FASTA data in 60-column lines.
#[inline(always)]
fn write<W: Write>(block: &[u8], output: &mut W) -> io::Result<()> {
    for chunk in block.chunks(LINE_LENGTH) {
        output.write_all(chunk)?;
        output.write_all(b"\n")?;
    }
    Ok(())
}

fn main() {
    let n = std::env::args_os().nth(1)
        .and_then(|s| s.into_string().ok())
        .and_then(|n| n.parse().ok())
        .unwrap_or(1000);

    let mut out = BufWriter::new(io::stdout());

    // Generate a DNA sequence by copying from the given sequence.
    const ALU: &[u8] =
        b"GGCCGGGCGCGGTGGCTCACGCCTGTAATCCCAGCACTTTGGGAGGCCGAGGCGGGCGGA\
          TCACCTGAGGTCAGGAGTTCGAGACCAGCCTGGCCAACATGGTGAAACCCCGTCTCTACT\
          AAAAATACAAAAATTAGCCGGGCGTGGTGGCGCGCGCCTGTAATCCCAGCTACTCGGGAG\
          GCTGAGGCAGGAGAATCGCTTGAACCCGGGAGGCGGAGGTTGCAGTGAGCCGAGATCGCG\
          CCACTGCACTCCAGCCTGGGCGACAGAGCGAGACTCCGTCTCAAAAA";
    let mut it = ALU.iter().cloned().cycle();

    out.write_all(b">ONE Homo sapiens alu\n").unwrap();
    make_fasta(n * 2, |block| for i in block.iter_mut() {
        *i = it.next().unwrap()
    }, &mut out);

    // Generate DNA sequences by weighted random selection from two alphabets.
    let p0 = cumulative_probabilities(
        &[('a', 0.27), ('c', 0.12), ('g', 0.12), ('t', 0.27), ('B', 0.02),
          ('D', 0.02), ('H', 0.02), ('K', 0.02), ('M', 0.02), ('N', 0.02),
          ('R', 0.02), ('S', 0.02), ('V', 0.02), ('W', 0.02), ('Y', 0.02)]);

    let p1 = cumulative_probabilities(
        &[('a', 0.3029549426680), ('c', 0.1979883004921),
          ('g', 0.1975473066391), ('t', 0.3015094502008)]);

    let mut rng = Rng::new();

    out.write_all(&b">TWO IUB ambiguity codes\n"[..]).unwrap();
    make_fasta(n * 3, |block| rng.gen(&p0, block), &mut out);

    out.write_all(&b">THREE Homo sapiens frequency\n"[..]).unwrap();
    make_fasta(n * 5, |block| rng.gen(&p1, block), &mut out);
}
