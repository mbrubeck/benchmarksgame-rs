// The Computer Language Benchmarks Game
// http://benchmarksgame.alioth.debian.org/
//
// contributed by the Rust Project Developers
// contributed by TeXitoi
// multi-threaded version contributed by Alisdair Owens

extern crate rayon;

use std::iter::repeat;
use std::io;
use std::io::{Write, BufWriter};
use std::sync::mpsc::{channel, Sender};
use std::thread;

const LINE_LENGTH: usize = 60;
const BLOCK_SIZE: usize = LINE_LENGTH * 1024;
const IM: u32 = 139968;

/// Pseudo-random number generator
struct MyRandom(u32);
impl MyRandom {
    fn new() -> Self { MyRandom(42) }

    fn gen(&mut self, probabilities: &[(u32, u8)], block: &mut [u8]) {
        for i in block.iter_mut() {
            self.0 = (self.0 * 3877 + 29573) % IM;
            *i = probabilities.iter().find(|&&(p, _)| p >= self.0).unwrap().1;
        }
    }
}

/// From a probability distribution, generate a cumulative probability distribution.
fn cumulative_probabilities(data: &[(char, f32)]) -> Vec<(u32, u8)> {
    fn normalize(p: f32) -> u32 {
        (p * IM as f32).floor() as u32
    }

    data.iter().scan(0., |acc, &(ch, p)| {
        *acc += p;
        Some((normalize(*acc), ch as u8))
    }).collect()
}

/// Number of rows required for `n` FASTA codes
fn num_lines(n: usize) -> usize { (n - 1) / LINE_LENGTH + 1 }

/// Output FASTA data from the provided generator function.
fn make_fasta<F: FnMut(&mut [u8])>(header: &str,
                                   out_thread: &Sender<Vec<u8>>,
                                   n: usize,
                                   mut gen: F)
{
    out_thread.send(header.to_string().into_bytes()).unwrap();

    /// Allocate a buffer with extra room for newlines
    fn buf(n: usize) -> Vec<u8> {
        let num_lines = num_lines(n);
        let mut buf = Vec::with_capacity(n + num_lines);
        unsafe { buf.set_len(n) }
        buf
    }

    // Write whole blocks.
    let num_blocks = n / BLOCK_SIZE;
    for _ in 0..num_blocks {
        let mut block = buf(BLOCK_SIZE);
        gen(&mut block);
        out_thread.send(block).unwrap();
    }

    // Write trailing block.
    let trailing_len = n % BLOCK_SIZE;
    if trailing_len > 0 {
        let mut block = buf(trailing_len);
        gen(&mut block);
        out_thread.send(block).unwrap();
    }
}

/// Wrap data to 60 columns.
fn format(block: &mut Vec<u8>) {
    let n = block.len();
    if n == 0 { return }

    let num_lines = num_lines(n);
    block.extend(repeat(b'\n').take(num_lines));

    let mut i = n - 1;
    let mut j = n - 2 + num_lines;

    while i >= LINE_LENGTH {
        block[j] = block[i];
        j -= 1;
        if i % LINE_LENGTH == 0 {
            block[j] = b'\n';
            j -= 1;
        }
        i -= 1;
    }
}

fn main() {
    let n = std::env::args_os().nth(1)
        .and_then(|s| s.into_string().ok())
        .and_then(|n| n.parse().ok())
        .unwrap_or(1000);

    let (tx0, rx0) = channel::<Vec<u8>>();
    let (tx1, rx1) = channel::<Vec<u8>>();

    // Generate a DNA sequence by copying from the given sequence.
    thread::spawn(move || {
        let alu: &[u8] = b"GGCCGGGCGCGGTGGCTCACGCCTGTAATCCCAGCACTTT\
                           GGGAGGCCGAGGCGGGCGGATCACCTGAGGTCAGGAGTTC\
                           GAGACCAGCCTGGCCAACATGGTGAAACCCCGTCTCTACT\
                           AAAAATACAAAAATTAGCCGGGCGTGGTGGCGCGCGCCTG\
                           TAATCCCAGCTACTCGGGAGGCTGAGGCAGGAGAATCGCT\
                           TGAACCCGGGAGGCGGAGGTTGCAGTGAGCCGAGATCGCG\
                           CCACTGCACTCCAGCCTGGGCGACAGAGCGAGACTCCGTCT\
                           CAAAAA";
        let mut it = alu.iter().cloned().cycle();

        make_fasta(">ONE Homo sapiens alu", &tx0, n * 2, |block| for i in block {
            *i = it.next().unwrap()
        });
    });

    // Generate DNA sequences by weighted random selection from two alphabets.
    thread::spawn(move || {
        let mut rng = MyRandom::new();
        let iub = cumulative_probabilities(
            &[('a', 0.27), ('c', 0.12), ('g', 0.12),
              ('t', 0.27), ('B', 0.02), ('D', 0.02),
              ('H', 0.02), ('K', 0.02), ('M', 0.02),
              ('N', 0.02), ('R', 0.02), ('S', 0.02),
              ('V', 0.02), ('W', 0.02), ('Y', 0.02)]);

        make_fasta(">TWO IUB ambiguity codes", &tx1, n * 3,
                   |block| rng.gen(&iub, block));

        let homosapiens = cumulative_probabilities(
            &[('a', 0.3029549426680), ('c', 0.1979883004921),
              ('g', 0.1975473066391), ('t', 0.3015094502008)]);

        make_fasta(">THREE Homo sapiens frequency", &tx1, n * 5,
                   |block| rng.gen(&homosapiens, block));
    });

    // Output blocks from the first thread, then the second one, as they are completed.
    let mut blocks = rx0.into_iter().chain(rx1);
    let mut output = BufWriter::new(io::stdout());

    while let Some(mut block0) = blocks.next() {
        if let Some(mut block1) = blocks.next() {
            if let Some(mut block2) = blocks.next() {
                if let Some(mut block3) = blocks.next() {
                    // Four threads
                    rayon::join(|| rayon::join(|| format(&mut block0), || format(&mut block1)),
                                || rayon::join(|| format(&mut block2), || format(&mut block3)));
                    output.write_all(&block0).unwrap();
                    output.write_all(&block1).unwrap();
                    output.write_all(&block2).unwrap();
                    output.write_all(&block3).unwrap();
                } else {
                    // Three threads
                    rayon::join(|| rayon::join(|| format(&mut block0),
                                               || format(&mut block1)),
                                || format(&mut block2));
                    output.write_all(&block0).unwrap();
                    output.write_all(&block1).unwrap();
                    output.write_all(&block2).unwrap();
                }
            } else {
                // Two threads
                rayon::join(|| format(&mut block0), || format(&mut block1));
                output.write_all(&block0).unwrap();
                output.write_all(&block1).unwrap();
            }
        } else {
            // One thread
            format(&mut block0);
            output.write_all(&block0).unwrap();
        }
    }
}
