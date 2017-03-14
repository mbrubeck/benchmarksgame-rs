// The Computer Language Benchmarks Game
// http://benchmarksgame.alioth.debian.org/
//
// contributed by the Rust Project Developers
// contributed by TeXitoi
// multi-threaded version contributed by Alisdair Owens

use std::io;
use std::io::{Write, BufWriter};
use std::sync::mpsc::{channel, Sender};
use std::thread;

const LINE_LENGTH: usize = 60;
const BLOCK_SIZE: usize = LINE_LENGTH * 1024;
const IM: u32 = 139968;

struct MyRandom(u32);

impl MyRandom {
    fn new() -> Self { MyRandom(42) }

    fn gen(&mut self, data: &[(u32, u8)], buf: &mut [u8]) {
        for i in buf.iter_mut() {
            self.0 = (self.0 * 3877 + 29573) % IM;
            for j in data {
                if j.0 >= self.0 {
                    *i = j.1;
                    break;
                }
            }
        }
    }
}

fn normalize(p: f32) -> u32 {
    (p * IM as f32).floor() as u32
}

fn make_random(data: &[(char, f32)]) -> Vec<(u32, u8)> {
    let mut acc = 0.;
    data.iter().map(|&(ch, p)| {
        acc += p;
        (normalize(acc), ch as u8)
    })
    .collect()
}

fn make_fasta2<I: Iterator<Item=u8>>(
    header: &str,
    out_thread: &Sender<Vec<u8>>,
    mut it: I,
    n: usize)
{
    out_thread.send(header.to_string().into_bytes()).unwrap();

    // Write whole blocks.
    let num_blocks = n / BLOCK_SIZE;
    for _ in 0..num_blocks {
        let mut buf = vec![0; BLOCK_SIZE];
        for i in &mut buf {
            *i = it.next().unwrap();
        }
        out_thread.send(buf).unwrap();
    }

    // Write trailing block.
    let trailing_len = n % BLOCK_SIZE;
    if trailing_len > 0 {
        let mut buf = vec![0; trailing_len];
        for i in &mut buf {
            *i = it.next().unwrap();
        }
        out_thread.send(buf).unwrap();
    }
}

fn make_fasta(
    header: &str,
    out_thread: &Sender<Vec<u8>>,
    n: usize,
    rng: &mut MyRandom,
    data: &[(u32, u8)])
{
    out_thread.send(header.to_string().into_bytes()).unwrap();

    // Write whole blocks.
    let num_blocks = n / BLOCK_SIZE;
    for _ in 0..num_blocks {
        let mut buf = vec![0; BLOCK_SIZE];
        rng.gen(data, &mut buf);
        out_thread.send(buf).unwrap();
    }

    // Write trailing block.
    let trailing_len = n % BLOCK_SIZE;
    if trailing_len > 0 {
        let mut buf = vec![0; trailing_len];
        rng.gen(data, &mut buf);
        out_thread.send(buf).unwrap();
    }
}

fn write<W: Write>(buf: &[u8], output: &mut W) -> io::Result<()> {
    let n = buf.len();
    let num_lines = n / LINE_LENGTH;

    // Write whole lines.
    for i in 0..num_lines {
        let start = i * LINE_LENGTH;
        let end = start + LINE_LENGTH;
        output.write_all(&buf[start..end])?;
        output.write_all(b"\n")?;
    }

    // Write trailing line.
    let trailing_len = n % LINE_LENGTH;
    if trailing_len > 0 {
        let start = num_lines * LINE_LENGTH;
        let end = start + trailing_len;
        output.write_all(&buf[start..end])?;
        output.write_all(b"\n")?;
    }
    Ok(())
}

fn main() {
    let n = std::env::args_os().nth(1)
        .and_then(|s| s.into_string().ok())
        .and_then(|n| n.parse().ok())
        .unwrap_or(1000);

    let (tx, rx) = channel::<Vec<u8>>();

    let output_thread = thread::spawn(move || {
        let mut output = BufWriter::new(io::stdout());
        while let Ok(block) = rx.recv() {
            write(&block, &mut output).unwrap();
        }
    });

    let alu: &[u8] = b"GGCCGGGCGCGGTGGCTCACGCCTGTAATCCCAGCACTTT\
                       GGGAGGCCGAGGCGGGCGGATCACCTGAGGTCAGGAGTTC\
                       GAGACCAGCCTGGCCAACATGGTGAAACCCCGTCTCTACT\
                       AAAAATACAAAAATTAGCCGGGCGTGGTGGCGCGCGCCTG\
                       TAATCCCAGCTACTCGGGAGGCTGAGGCAGGAGAATCGCT\
                       TGAACCCGGGAGGCGGAGGTTGCAGTGAGCCGAGATCGCG\
                       CCACTGCACTCCAGCCTGGGCGACAGAGCGAGACTCCGTCT\
                       CAAAAA";

    make_fasta2(">ONE Homo sapiens alu", &tx,
                    alu.iter().cloned().cycle(), n * 2);

    let iub = &[('a', 0.27), ('c', 0.12), ('g', 0.12),
                ('t', 0.27), ('B', 0.02), ('D', 0.02),
                ('H', 0.02), ('K', 0.02), ('M', 0.02),
                ('N', 0.02), ('R', 0.02), ('S', 0.02),
                ('V', 0.02), ('W', 0.02), ('Y', 0.02)];

    let homosapiens = &[('a', 0.3029549426680),
                        ('c', 0.1979883004921),
                        ('g', 0.1975473066391),
                        ('t', 0.3015094502008)];

    let mut rng = MyRandom::new();

    make_fasta(">TWO IUB ambiguity codes", &tx, n * 3,
                    &mut rng, &make_random(iub));

    make_fasta(">THREE Homo sapiens frequency", &tx, n * 5,
                    &mut rng, &make_random(homosapiens));
    drop(tx);

    output_thread.join().unwrap();
}
