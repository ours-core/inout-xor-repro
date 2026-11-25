//! `StreamCipher::apply_keystream` against the same keystream XORed by hand a word at a time.
use std::hint::black_box;
use std::time::Instant;

use ctr::cipher::{KeyIvInit, StreamCipher};

type Ctr = ctr::Ctr128BE<aes::Aes256>;

const PIECE: usize = 16 * 1024;
const TOTAL: usize = 1 << 30;

fn cipher() -> Ctr {
    Ctr::new(&[1; 32].into(), &[2; 16].into())
}

/// The keystream from `write_keystream`, XORed 16 bytes at a time.
fn apply_by_words(cipher: &mut Ctr, data: &mut [u8]) {
    let mut stream = [0u8; 2048];
    for chunk in data.chunks_mut(stream.len()) {
        let stream = &mut stream[..chunk.len()];
        cipher.write_keystream(stream);
        let mut words = chunk.chunks_exact_mut(16);
        let mut keys = stream.chunks_exact(16);
        for (word, key) in (&mut words).zip(&mut keys) {
            let x = u128::from_ne_bytes((*word).try_into().unwrap())
                ^ u128::from_ne_bytes(key.try_into().unwrap());
            word.copy_from_slice(&x.to_ne_bytes());
        }
        for (byte, key) in words.into_remainder().iter_mut().zip(keys.remainder()) {
            *byte ^= key;
        }
    }
}

fn rate(f: impl Fn(&mut [u8])) -> f64 {
    let mut data = vec![0u8; PIECE];
    let mut best = 0f64;
    for _ in 0..5 {
        let start = Instant::now();
        for _ in 0..TOTAL / PIECE {
            f(black_box(&mut data));
        }
        best = best.max(TOTAL as f64 / start.elapsed().as_secs_f64() / 1e9);
    }
    best
}

fn main() {
    let (mut a, mut b) = (vec![7u8; 5000], vec![7u8; 5000]);
    cipher().apply_keystream(&mut a);
    apply_by_words(&mut cipher(), &mut b);
    assert_eq!(a, b, "both ways give the same bytes");

    let c = std::cell::RefCell::new(cipher());
    let apply = rate(|data| c.borrow_mut().apply_keystream(data));
    let c = std::cell::RefCell::new(cipher());
    let words = rate(|data| apply_by_words(&mut c.borrow_mut(), data));
    println!("apply_keystream: {apply:.2} GB/s");
    println!("by words:        {words:.2} GB/s");
}
