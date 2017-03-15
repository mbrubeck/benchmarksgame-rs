// The Computer Language Benchmarks Game
// http://benchmarksgame.alioth.debian.org/
//
// contributed by Matt Brubeck

use std::io;
use std::io::{Write, BufWriter};
use std::sync::mpsc::{channel, Sender};
use std::thread;

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
fn make_fasta<F: FnMut(&mut [u8])>(header: &str,
                                   out_thread: &Sender<Vec<u8>>,
                                   n: usize,
                                   mut gen: F)
{
    out_thread.send(header.to_string().into_bytes()).unwrap();

    // Write whole blocks.
    let num_blocks = n / BLOCK_SIZE;
    for _ in 0..num_blocks {
        let mut buf = vec![0; BLOCK_SIZE];
        gen(&mut buf);
        out_thread.send(buf).unwrap();
    }

    // Write trailing block.
    let trailing_len = n % BLOCK_SIZE;
    if trailing_len > 0 {
        let mut buf = vec![0; trailing_len];
        gen(&mut buf);
        out_thread.send(buf).unwrap();
    }
}

/// Print FASTA data in 60-column lines.
fn write<W: Write>(buf: &[u8], output: &mut W) -> io::Result<()> {
    let n = buf.len();
    let mut start = 0;

    while start < n {
        let end = std::cmp::min(start + LINE_LENGTH, n);
        output.write_all(&buf[start..end])?;
        output.write_all(b"\n")?;
        start = end;
    }
    Ok(())
}

fn main() {
    let n = std::env::args_os().nth(1)
        .and_then(|s| s.into_string().ok())
        .and_then(|n| n.parse().ok())
        .unwrap_or(1000);

    // Generate a DNA sequence by copying from the given sequence.
    let (tx, rx0) = channel();
    thread::spawn(move || {
        const ALU: &[u8] =
            b"GGCCGGGCGCGGTGGCTCACGCCTGTAATCCCAGCACTTTGGGAGGCCGAGGCGGGCGGA\
              TCACCTGAGGTCAGGAGTTCGAGACCAGCCTGGCCAACATGGTGAAACCCCGTCTCTACT\
              AAAAATACAAAAATTAGCCGGGCGTGGTGGCGCGCGCCTGTAATCCCAGCTACTCGGGAG\
              GCTGAGGCAGGAGAATCGCTTGAACCCGGGAGGCGGAGGTTGCAGTGAGCCGAGATCGCG\
              CCACTGCACTCCAGCCTGGGCGACAGAGCGAGACTCCGTCTCAAAAA";
        let mut it = ALU.iter().cloned().cycle();

        make_fasta(">ONE Homo sapiens alu", &tx, n * 2, |buf| for i in buf {
            *i = it.next().unwrap()
        });
    });

    // Generate DNA sequences by weighted random selection from two alphabets.
    let (tx, rx1) = channel();
    thread::spawn(move || {
        let p0 = cumulative_probabilities(
            &[('a', 0.27), ('c', 0.12), ('g', 0.12), ('t', 0.27), ('B', 0.02),
              ('D', 0.02), ('H', 0.02), ('K', 0.02), ('M', 0.02), ('N', 0.02),
              ('R', 0.02), ('S', 0.02), ('V', 0.02), ('W', 0.02), ('Y', 0.02)]);

        let p1 = cumulative_probabilities(
            &[('a', 0.3029549426680), ('c', 0.1979883004921),
              ('g', 0.1975473066391), ('t', 0.3015094502008)]);

        let mut rng = Rng::new();

        make_fasta(">TWO IUB ambiguity codes",      &tx, n * 3, |buf| rng.gen(&p0, buf));
        make_fasta(">THREE Homo sapiens frequency", &tx, n * 5, |buf| rng.gen(&p1, buf));
    });

    // Output completed blocks from the first thread, then the second one.
    let mut output = BufWriter::new(io::stdout());
    for block in rx0.into_iter().chain(rx1) {
        write(&block, &mut output).unwrap();
    }
}
